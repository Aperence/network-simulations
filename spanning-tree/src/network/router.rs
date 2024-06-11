use std::{cell::RefCell, collections::{hash_map::Entry, BinaryHeap, HashMap, HashSet}, net::Ipv4Addr, rc::Rc, sync::Arc, time::SystemTime};
use log::info;
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

use super::messages::{ospf::OSPFMessage, Message};
use super::messages::ospf::OSPFMessage::{Hello, HelloReply, LSP};
use super::communicators::{RouterCommunicator, Command, Response};

#[derive(Ord, PartialEq, Eq, Hash, Clone)]
pub struct Node{
    distance: u32,
    ip: Ipv4Addr,
    port: u32
}

impl PartialOrd for Node{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

type Neighbor = (Arc<Mutex<Receiver<Message>>>, Sender<Message>, u32); // receiver, sender, cost

#[derive(Debug)]
pub struct Router{
    pub name: String,
    pub ip: Ipv4Addr,
    pub id: u32,
    pub neighbors: HashMap<u32, Neighbor>, 
    pub command_receiver: Receiver<Command>,
    pub command_replier: Sender<Response>,
    pub topo: HashMap<Ipv4Addr, HashSet<(u32, Ipv4Addr)>>,
    pub direct_neighbors: HashSet<(u32, u32, Ipv4Addr)>,
    pub routing_table: HashMap<Ipv4Addr, (u32, u32)>,
    pub received_lsp: HashSet<(Ipv4Addr, u32)>,
    pub lsp_seq: u32
}

impl ToString for Router{
    fn to_string(&self) -> String{
        format!("Router {}", self.name)
    }
}

impl Router{

    pub fn start(name: String, id: u32) -> RouterCommunicator{
        let (tx_command, rx_command) = channel(1024);
        let (tx_response, rx_response) = channel(1024);
        let mut router = Router{
            name, 
            ip: Ipv4Addr::new(10, 0, 0, id as u8),
            id, 
            neighbors: HashMap::new(), 
            command_receiver: rx_command,
            command_replier: tx_response,
            topo: HashMap::new(),
            direct_neighbors: HashSet::new(),
            routing_table: HashMap::new(),
            received_lsp: HashSet::new(),
            lsp_seq: 0
        };
        router.routing_table.insert(router.ip, (0, 0));
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
                // every 200ms, send an hello message
                time = SystemTime::now();
                self.send_hello().await;
            }
            
        }
    }

    pub async fn receive_messages(&mut self){
        let mut received_messages = vec![];
        for (port, (receiver, _, cost)) in self.neighbors.iter(){
            let mut receiver = receiver.lock().await;
            if let Ok(message) = receiver.try_recv(){
                received_messages.push((message, *port, *cost));
            }
        }
        for (message, port, _cost) in received_messages{
            info!("Router {} received {:?}", self.name, message);
            match message{
                Message::BPDU(_) => (), // don't care about bdpus
                Message::OSPF(ospf) => self.process_ospf(ospf, port).await,
            }
        }
    }

    pub async fn process_ospf(&mut self, ospf: OSPFMessage, port: u32){
        match ospf{
            Hello => self.send_hello_reply(port).await,
            LSP(from, seq, neighbors) => self.process_lsp(from, seq, neighbors).await,
            HelloReply(ip) => self.process_hello_reply(ip, port).await,
        }
    }

    pub async fn shortest_path(&mut self){
        let mut visited = HashSet::new();
        let mut pq = BinaryHeap::new();

        visited.insert(self.ip);
        for (cost, port, ip) in self.direct_neighbors.iter(){
            pq.push(Node{distance: *cost, ip: ip.clone(), port: *port});
        }

        while !pq.is_empty(){
            let p = pq.pop().unwrap();
            if visited.contains(&p.ip){
                continue;
            }
            self.routing_table.insert(p.ip, (p.port, p.distance));
            visited.insert(p.ip.clone());
            let neighs = self.topo.get(&p.ip);
            if let Some(n) = neighs{
                for (cost, neigh) in n{
                    pq.push(Node{distance: p.distance+cost, ip: *neigh, port: p.port});
                }
            }
        }
        info!("Router {} has updated its routing table : {:?}", self.name, self.routing_table);
    }

    pub async fn process_lsp(&mut self, from: Ipv4Addr, seq: u32, neighbors: HashSet<(u32, Ipv4Addr)>){
        if self.received_lsp.contains(&(from, seq)){
            return;
        }
        self.received_lsp.insert((from, seq));
        let values = match self.topo.entry(from) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        values.extend(neighbors.iter());
        self.shortest_path().await;
        info!("Topology of {} : {:?}", self.name, self.topo);

        self.send_lsp(OSPFMessage::LSP(from, seq, neighbors)).await; // flood
    }

    pub async fn send_lsp(&mut self, lsp: OSPFMessage){
        for (port, (_, sender, _)) in self.neighbors.iter() {
            info!("Router {} sending {:?} on port {}", self.name, lsp, port);
            sender.send(Message::OSPF(lsp.clone())).await.unwrap();
        }
    }

    pub async fn process_hello_reply(&mut self, ip: Ipv4Addr, port: u32){
        let (_, _, cost) = self.neighbors.get(&port).unwrap();
        self.direct_neighbors.insert((*cost, port, ip));
        info!("Router {} has neighbors : {:?}", self.name, self.direct_neighbors);
        self.routing_table.insert(ip, (port, *cost));

        let values = match self.topo.entry(self.ip) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        values.insert((*cost, ip));
        
        info!("Router {} received prefix {} from neighbor on port {}", self.name, ip, port);
        let seq = self.lsp_seq;
        self.lsp_seq+=1;
        let mut neighs = HashSet::new();
        for (cost, _port, n) in self.direct_neighbors.iter(){
            neighs.insert((*cost, n.clone()));
        }
        self.send_lsp(OSPFMessage::LSP(self.ip, seq, neighs)).await;
    }

    pub async fn send_hello(&self){
        for (port, (_, sender, _)) in self.neighbors.iter() {
            let msg = Message::OSPF(Hello);
            info!("Router {} sending Hello on port {}", self.name, port);
            sender.send(msg).await.unwrap();
        }
    }

    pub async fn send_hello_reply(&self, port: u32){
        let (_, sender, _) = self.neighbors.get(&port).unwrap();
        info!("Router {} sending hello reply on port {}", self.name, port);
        sender.send(Message::OSPF(OSPFMessage::HelloReply(self.ip))).await.expect("Failed to send Hello reply");
    }

    pub async fn receive_command(&mut self) -> bool{
        match self.command_receiver.try_recv(){
            Ok(command) => {
                match command{
                    Command::AddLink(receiver, sender, port, cost) => {
                        info!("Router {} received adding link", self.name);
                        let receiver = Arc::new(Mutex::new(receiver));
                        self.neighbors.insert(port, (receiver, sender, cost));
                        false
                    },
                    Command::Quit => true,
                    Command::StatePorts => panic!("Unsupported command"),
                }
            },
            Err(_) => false,
        }
    }
}