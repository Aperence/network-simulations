use std::{cell::RefCell, collections::HashMap, net::Ipv4Addr, rc::Rc, sync::Arc, time::SystemTime};
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

use super::{logger::{Logger, Source}, messages::{DebugMessage, Message}};
use super::communicators::{RouterCommunicator, Command, Response};
use super::protocols::ospf::OSPFState;

type Neighbor = (Arc<Mutex<Receiver<Message>>>, Sender<Message>, u32); // receiver, sender, cost

#[derive(Debug)]
pub struct RouterInfo{
    pub name: String,
    pub id: u32,
    pub ip: Ipv4Addr,
    pub neighbors: HashMap<u32, Neighbor>,
}

#[derive(Debug)]
pub struct Router{
    pub router_info: Arc<Mutex<RouterInfo>>,
    pub command_receiver: Receiver<Command>,
    pub command_replier: Sender<Response>,
    pub igp_state: Arc<Mutex<OSPFState>>,
    pub logger: Logger
}

impl Router{

    pub fn start(name: String, id: u32, logger: Logger) -> RouterCommunicator{
        let (tx_command, rx_command) = channel(1024);
        let (tx_response, rx_response) = channel(1024);
        let ip = Ipv4Addr::new(10, 0, 0, id as u8);
        let router_info = Arc::new(Mutex::new(RouterInfo{
            name, 
            ip,
            id, 
            neighbors: HashMap::new(), 
        }));
        let mut router = Router{
            router_info: Arc::clone(&router_info),
            command_receiver: rx_command,
            command_replier: tx_response,
            igp_state: Arc::new(Mutex::new(OSPFState::new(ip, logger.clone(), router_info))),
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
                // every 200ms, send an hello message
                time = SystemTime::now();
                self.igp_state.lock().await.send_hello().await;
            }
            
        }
    }

    pub async fn receive_messages(&mut self){
        let mut received_messages = vec![];
        let info = self.router_info.lock().await;
        for (port, (receiver, _, cost)) in info.neighbors.iter(){
            let mut receiver = receiver.lock().await;
            if let Ok(message) = receiver.try_recv(){
                received_messages.push((message, *port, *cost));
            }
        }
        let name = info.name.clone();
        drop(info);
        for (message, port, _cost) in received_messages{
            self.logger.log(Source::Debug, format!("Router {} received {:?}", name, message)).await;
            
            match message{
                Message::BPDU(_) => (), // don't care about bdpus
                Message::OSPF(ospf) => self.igp_state.lock().await.process_ospf(ospf, port).await,
                Message::Debug(debug) => self.process_debug(debug).await,
            }
        }
    }

    pub async fn process_debug(&self, debug: DebugMessage){
        let info = self.router_info.lock().await;
        let ip = info.ip.clone();
        let name = info.name.clone();
        drop(info);
        match debug{
            DebugMessage::Ping(from, to) => {
                if to == ip{
                    self.logger.log(Source::Ping, format!("Router {} received ping from {}", name, from)).await;
                    self.send_message(from, Message::Debug(DebugMessage::Pong(to, from))).await;
                }else{
                    self.send_message(to, Message::Debug(debug)).await;
                }
            },
            DebugMessage::Pong(_, to) => {
                if to == ip{
                    self.logger.log(Source::Ping, format!("Router {} received ping back from {}", name, to)).await;
                    return
                }
                self.send_message(to, Message::Debug(debug)).await;
            },
        }
    }

    pub async fn send_message(&self, dest: Ipv4Addr, message: Message){
        let info = self.router_info.lock().await;
        if let Some((port, _)) = self.igp_state.lock().await.get_port(dest){
            let (_, sender, _) = info.neighbors.get(port).unwrap();
            sender.send(message).await.unwrap();
        }
    }

    pub async fn send_ping(&self, dest: Ipv4Addr){
        let info = self.router_info.lock().await;
        self.logger.log(Source::Ping, format!("Router {} sending ping message to {}", info.name, dest)).await;
        self.send_message(dest, Message::Debug(DebugMessage::Ping(info.ip, dest))).await;
    }

    pub async fn receive_command(&mut self) -> bool{
        match self.command_receiver.try_recv(){
            Ok(command) => {
                match command{
                    Command::AddLink(receiver, sender, port, cost) => {
                        let mut info = self.router_info.lock().await;
                        self.logger.log(Source::Debug, format!("Router {} received adding link", info.name)).await;
                        let receiver = Arc::new(Mutex::new(receiver));
                        info.neighbors.insert(port, (receiver, sender, cost));
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
                }
            },
            Err(_) => false,
        }
    }
}