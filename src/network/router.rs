use std::{cell::RefCell, collections::HashMap, net::Ipv4Addr, rc::Rc, sync::Arc, time::SystemTime};
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

use super::{ip_prefix::IPPrefix, logger::{Logger, Source}, messages::{ip::{Content, IP}, Message}, protocols::{arp::ArpState, bgp::BGPState}, utils::{MacAddress, SharedState}};
use super::communicators::{RouterCommunicator, Command, Response};
use super::protocols::ospf::OSPFState;

type Neighbor = (SharedState<Receiver<Message>>, Sender<Message>); // receiver, sender

type BGPNeighbor = (u32, u32); // pref, med

type IGPNeighbor = u32;  // cost

#[derive(Debug)]
pub struct RouterInfo{
    pub name: String,
    pub id: u32,
    pub router_as: u32,
    pub ip: Ipv4Addr,
    pub mac_address: MacAddress,
    pub neighbors_links: HashMap<u32, Neighbor>,
    pub igp_links: HashMap<u32, IGPNeighbor>,
    pub bgp_links: HashMap<u32, BGPNeighbor>,
    pub ibgp_peers: Vec<Ipv4Addr>
}

#[derive(Debug)]
pub struct Router{
    pub router_info: SharedState<RouterInfo>,
    pub command_receiver: Receiver<Command>,
    pub command_replier: Sender<Response>,
    pub igp_state: SharedState<OSPFState>,
    pub arp_state: SharedState<ArpState>,
    pub bgp_state: SharedState<BGPState>,
    pub logger: Logger
}

impl Router{

    pub fn start(name: String, id: u32, router_as: u32, logger: Logger) -> RouterCommunicator{
        let (tx_command, rx_command) = channel(1024);
        let (tx_response, rx_response) = channel(1024);
        let ip = Ipv4Addr::new(10, 0, router_as as u8, id as u8);
        let router_info = Arc::new(Mutex::new(RouterInfo{
            name, 
            ip,
            id, 
            mac_address: MacAddress{id},
            router_as,
            neighbors_links: HashMap::new(), 
            igp_links: HashMap::new(),
            bgp_links: HashMap::new(),
            ibgp_peers: vec![]
        }));
        let arp_state = Arc::new(Mutex::new(ArpState::new(Arc::clone(&router_info), logger.clone())));
        let igp_state = Arc::new(Mutex::new(OSPFState::new(ip, logger.clone(), Arc::clone(&router_info), Arc::clone(&arp_state))));
        let mut router = Router{
            router_info: Arc::clone(&router_info),
            command_receiver: rx_command,
            command_replier: tx_response,
            igp_state: Arc::clone(&igp_state) ,
            arp_state,
            bgp_state: Arc::new(Mutex::new(BGPState::new(router_info, igp_state, logger.clone()))),
            logger
        };
        tokio::spawn(async move {
            router.run().await;
        });
        RouterCommunicator{command_sender: tx_command, response_receiver: Rc::new(RefCell::new(rx_response))}
    }

    pub async fn run(&mut self){
        let mut time = SystemTime::now();
        loop{
            if self.receive_command().await{
                return;
            }
            self.receive_messages().await;
            if time.elapsed().unwrap().as_millis() > 200{
                // every 200ms, send an hello message, and refresh arp state
                time = SystemTime::now();
                let igp_state = self.igp_state.lock().await;
                igp_state.send_hello().await;
                let arp_state = self.arp_state.lock().await;
                for (_, port, ip) in igp_state.direct_neighbors.iter(){
                    arp_state.resolve(ip.ip, *port).await;
                }
            }
        }
    }

    pub async fn receive_messages(&mut self){
        let mut received_messages = vec![];
        let info = self.router_info.lock().await;
        for (port, (receiver, _)) in info.neighbors_links.iter(){
            let mut receiver = receiver.lock().await;
            if let Ok(message) = receiver.try_recv(){
                received_messages.push((message, *port));
            }
        }
        let name = info.name.clone();
        drop(info);
        for (message, port) in received_messages{
            self.logger.log(Source::DEBUG, format!("Router {} received {:?}", name, message)).await;
            
            match message{
                Message::BPDU(_) => (), // don't care about bdpus
                Message::OSPF(ospf) => self.igp_state.lock().await.process_ospf(ospf, port).await,
                Message::EthernetFrame(mac, ip) => self.process_frame(port, mac, ip).await,
                Message::BGP(bgp_message) => self.bgp_state.lock().await.process_bgp_message(port, bgp_message).await,
                Message::ARP(arp_message) => self.arp_state.lock().await.process_arp_message(arp_message, port).await,
            }
        }
    }

    pub async fn process_frame(&self,port: u32, mac: MacAddress, content: IP){
        let self_mac = self.router_info.lock().await.mac_address.clone();
        if self_mac == mac{
            self.process_ip(port, content).await;
        }
    }

    pub async fn process_ip(&self, port: u32, ip_packet: IP){
        let info = self.router_info.lock().await;
        let ip = info.ip.clone();
        self.logger.log(Source::IP, format!("Router {} received ip packet {:?}", info.name, ip_packet)).await;
        drop(info);
        if ip_packet.dest == ip{
            self.process_ip_content(port, ip_packet).await;
        }else{
            self.send_message(ip_packet.dest, ip_packet).await;
        }
    }

    pub async fn process_ip_content(&self, port: u32, ip_packet: IP){
        let info = self.router_info.lock().await;
        let ip = info.ip.clone();
        let name = info.name.clone();
        drop(info);
        match ip_packet.content{
            Content::Ping => {
                self.logger.log(Source::PING, format!("Router {} received ping from {}", name, ip_packet.src)).await;
                self.send_message(ip_packet.src, IP{src: ip, dest: ip_packet.src, content: Content::Pong}).await;
            },
            Content::Pong => {
                self.logger.log(Source::PING, format!("Router {} received ping back from {}", name, ip_packet.src)).await;
            },
            Content::Data(data) => {
                self.logger.log(Source::IP, format!("Router {} received data {} from {}", name, data, ip_packet.src)).await;
            },
            Content::IBGP(ibgp_message) => {
                self.bgp_state.lock().await.process_ibgp_message(port, ibgp_message).await
            },
        }
    }

    pub async fn send_message(&self, dest: Ipv4Addr, message: IP){
        let bgp_state = self.bgp_state.lock().await;
        if let Some(nexthop) = bgp_state.get_nexthop(dest).await{
            self.igp_state.lock().await.send_message(nexthop, message).await;
        }else{
            self.igp_state.lock().await.send_message(message.dest, message).await;
        }
    }

    pub async fn send_ping(&self, dest: Ipv4Addr){
        let info = self.router_info.lock().await;
        let src = info.ip.clone();
        let name = info.name.clone();
        drop(info);
        self.logger.log(Source::PING, format!("Router {} sending ping message to {}", name, dest)).await;
        self.send_message(dest, IP{src, dest, content: Content::Ping}).await;
    }

    pub async fn receive_command(&mut self) -> bool{
        match self.command_receiver.try_recv(){
            Ok(command) => {
                match command{
                    Command::AddLink(receiver, sender, port, cost) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::DEBUG, format!("Router {} received adding link", info.name)).await;
                        let receiver = Arc::new(Mutex::new(receiver));
                        info.neighbors_links.insert(port, (receiver, sender));
                        info.igp_links.insert(port, cost);
                        false
                    },
                    Command::Quit => true,
                    Command::StatePorts => panic!("Unsupported command"),
                    Command::Ping(dest) => {
                        self.send_ping(dest).await;
                        false
                    },
                    Command::RoutingTable => {
                        self.command_replier.send(Response::RoutingTable(self.igp_state.lock().await.routing_table.clone())).await.expect("Failed to send the routing table");
                        false
                    },
                    Command::AddPeerLink(receiver, sender, port, med, other_ip) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::DEBUG, format!("Router {} received adding peer link", info.name)).await;
                        let receiver = Arc::new(Mutex::new(receiver));
                        info.neighbors_links.insert(port, (receiver, sender));
                        info.bgp_links.insert(port, (100, med));
                        let prefix = IPPrefix{ip: other_ip, prefix_len: 32};
                        let mut igp_state = self.igp_state.lock().await;
                        igp_state.routing_table.insert(prefix, (port, 1));
                        igp_state.prefixes.insert(prefix, prefix);
                        igp_state.direct_neighbors.insert((1, port, prefix));
                        false
                    },
                    Command::AddProvider(receiver, sender, port, med, other_ip) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::DEBUG, format!("Router {} received adding provider link", info.name)).await;
                        let receiver = Arc::new(Mutex::new(receiver));
                        info.neighbors_links.insert(port, (receiver, sender));
                        info.bgp_links.insert(port, (50, med));
                        let prefix = IPPrefix{ip: other_ip, prefix_len: 32};
                        let mut igp_state = self.igp_state.lock().await;
                        igp_state.routing_table.insert(prefix, (port, 1));
                        igp_state.prefixes.insert(prefix, prefix);
                        igp_state.direct_neighbors.insert((1, port, prefix));
                        false
                    },
                    Command::AddCustomer(receiver, sender, port, med, other_ip) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::DEBUG, format!("Router {} received adding customer link", info.name)).await;
                        let receiver = Arc::new(Mutex::new(receiver));
                        info.neighbors_links.insert(port, (receiver, sender));
                        info.bgp_links.insert(port, (150, med));
                        let prefix = IPPrefix{ip: other_ip, prefix_len: 32};
                        let mut igp_state = self.igp_state.lock().await;
                        igp_state.routing_table.insert(prefix, (port, 1));
                        igp_state.prefixes.insert(prefix, prefix);
                        igp_state.direct_neighbors.insert((1, port, prefix));
                        false
                    },
                    Command::AnnouncePrefix => {
                        self.bgp_state.lock().await.announce_prefix().await;
                        false
                    },
                    Command::BGPRoutes => {
                        let bgp_state = self.bgp_state.lock().await;
                        let mut routes = HashMap::new();
                        
                        for (prefix, r) in bgp_state.routes.iter(){
                            let best_route = bgp_state.decision_process(*prefix).await;
                            routes.insert(*prefix, (best_route, r.clone()));
                        }
                        self.command_replier.send(Response::BGPRoutes(routes)).await.expect("Failed to send the routing table");
                        false
                    },
                    Command::AddIBGP(peer_addr) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::DEBUG, format!("Router {} received adding ibp connection to {}", info.name, peer_addr)).await;
                        info.ibgp_peers.push(peer_addr);
                        false
                    },
                }
            },
            Err(_) => false,
        }
    }
}