use std::{cell::RefCell, collections::{BTreeMap, HashMap}, rc::Rc, sync::Arc, time::SystemTime};
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

use super::{logger::{Logger, Source}, messages::{bpdu::BPDU, Message}};
use super::communicators::{SwitchCommunicator, Command, Response};

#[derive(Debug, Clone, PartialEq)]
pub enum PortState{
    Blocked,
    Designated,
    Root
}

impl ToString for PortState{
    fn to_string(&self) -> String{
        match self {
            PortState::Blocked => "B".into(),
            PortState::Designated => "D".into(),
            PortState::Root => "R".into(),
        }
    }
}

type Neighbor = (u32, Arc<Mutex<Receiver<Message>>>, Sender<Message>, u32); // port, receiver, sender, cost

#[derive(Debug)]
pub struct Switch{
    pub name: String,
    pub id: u32,
    pub neighbors: Vec<Neighbor>, 
    pub bpdu: BPDU,
    pub root_port: u32,
    pub ports: HashMap<u32, (BPDU, u32)>,
    pub ports_states: HashMap<u32, PortState>,
    pub command_receiver: Receiver<Command>,
    pub command_replier: Sender<Response>,
    pub logger: Logger
}

impl ToString for Switch{
    fn to_string(&self) -> String{
        format!("Switch {}", self.name)
    }
}

impl Switch{

    pub fn start(name: String, id: u32, logger: Logger) -> SwitchCommunicator{
        let (tx_command, rx_command) = channel(1024);
        let (tx_response, rx_response) = channel(1024);
        let mut switch = Switch{
            name, 
            id, 
            neighbors: vec![], 
            ports: HashMap::new(), 
            ports_states: HashMap::new(), 
            root_port: 0, 
            bpdu: BPDU{root: id, distance: 0, switch: id, port: 0}, 
            command_receiver: rx_command,
            command_replier: tx_response,
            logger
        };
        tokio::spawn(async move {
            switch.run().await;
        });
        SwitchCommunicator{command_sender: tx_command, response_receiver: Rc::new(RefCell::new(rx_response))}
    }

    pub async fn run(&mut self){
        self.logger.log(Source::SPT, format!("Init BPDU for switch {} : {}", self.name, self.bpdu.to_string())).await;
        let mut time = SystemTime::now();
        loop{
            if self.receive_command().await{
                return;
            }
            self.receive_ports().await;
            if time.elapsed().unwrap().as_millis() > 200{
                // every 200ms, send my own bpdu
                time = SystemTime::now();
                self.send_bpdu().await;
            }
            
        }
    }

    pub async fn receive_command(&mut self) -> bool{
        match self.command_receiver.try_recv(){
            Ok(command) => {
                match command{
                    Command::StatePorts => {
                        let mut map = BTreeMap::new();
                        for (port, state) in self.ports_states.iter(){
                            map.insert(*port, state.clone());
                        }
                        self.command_replier.send(Response::StatePorts(map)).await.expect("Failed to send response to state port command");
                        false
                    },
                    Command::AddLink(receiver, sender, port, cost) => {
                        let receiver = Arc::new(Mutex::new(receiver));
                        self.neighbors.push((port, receiver, sender, cost));
                        self.ports_states.insert(port, PortState::Designated);
                        false
                    },
                    Command::Quit => true,
                    Command::Ping(_) => panic!("Ping not supported on switch"),
                    Command::RoutingTable => panic!("RoutingTable not supported on switch"),
                    Command::AddPeerLink(_, _, _, _, _) => panic!("Adding peer link not supported on switch"),
                    Command::AddProvider(_, _, _, _, _) => panic!("Adding provider link not supported on switch"),
                    Command::AddCustomer(_, _, _, _, _) => panic!("Adding customer link not supported on switch"),
                    Command::AnnouncePrefix => panic!("Announcing prefix not supported on switch"),
                    Command::BGPRoutes => panic!("BGPRoutes not supported on switch"),
                }
            },
            Err(_) => false,
        }
    }

    pub async fn receive_ports(&mut self){
        let mut received_bpdus = vec![];
        let mut received_messages= vec![];
        for (port, receiver, _, cost) in self.neighbors.iter(){
            let mut receiver = receiver.lock().await;
            match receiver.try_recv(){
                Ok(Message::BPDU(bpdu)) => received_bpdus.push((bpdu, *port, *cost)),
                Ok(message) => {
                    if self.get_port_state(*port) != PortState::Blocked{
                        received_messages.push((*port, message))
                    }
                }
                Err(_) => continue,
            }
        }
        for (bpdu, port, cost) in received_bpdus{
            self.receive_bpdu(bpdu, port, cost).await;
        }
        for (port, message) in received_messages{
            for (p, _, sender, _) in self.neighbors.iter(){
                if port != *p && self.get_port_state(*p) != PortState::Blocked{
                    sender.send(message.clone()).await.expect("Failed to broadcast message");
                }
            }
        }
    }

    pub async fn receive_bpdu(&mut self, bpdu: BPDU, port: u32, distance: u32){
        self.logger.log(Source::SPT, format!("Switch {} received BPDU {} on port {}", self.name, bpdu.to_string(), port)).await;
        let prev = self.ports.get(&port);
        if let Some((prev_bpdu, _)) = prev{
            if prev_bpdu < &bpdu{
                return;
            }
        }
        self.ports.insert(port, (bpdu.clone(), distance));
        self.update_best(BPDU{root: bpdu.root, distance: bpdu.distance+distance, switch: bpdu.switch, port: bpdu.port}, port).await;
        self.update_state_port(port).await;
        // updated root, resend my bpdu to all neighbors
        if self.root_port == port{
            self.send_bpdu().await;
        }
    }

    async fn update_state_port(&mut self, port: u32){
        let bpdu = self.ports.get(&port);
        if bpdu.is_none(){
            return;
        }
        let (bpdu, _) = bpdu.unwrap();
        if port == self.root_port{
            self.ports_states.insert(port, PortState::Root);
        }else if bpdu < &self.bpdu{
            self.logger.log(Source::SPT, format!("BPDU received ({}) by {} on port {} was better than self bpdu ({}), port {} becomes blocked", bpdu.to_string(), self.name, port, self.bpdu.to_string(), port)).await;
            self.ports_states.insert(port, PortState::Blocked);
        }else{
            self.logger.log(Source::SPT, format!("BPDU received ({}) by {} on port {} was worse than self bpdu ({}), port {} becomes designated", bpdu.to_string(), self.name, port, self.bpdu.to_string(), port)).await;
            self.ports_states.insert(port, PortState::Designated);
        }
    }

    pub async fn send_bpdu(&self){
        for (port, _, sender, _) in self.neighbors.iter() {
            if self.get_port_state(*port) != PortState::Designated{
                // either we can't send a bpdu on this port, or it generated a cycle for rust borrows, no point to continue
                continue;
            }
            let bpdu = BPDU{root: self.bpdu.root, distance: self.bpdu.distance, switch: self.id, port: *port};
            self.logger.log(Source::SPT, format!("Switch {} sending BPDU {} on port {}", self.name, bpdu.to_string(), port)).await;
            sender.send(Message::BPDU(bpdu)).await.unwrap();
        }
    }

    fn get_ports(&self) -> Vec<u32>{
        let mut ports = vec![];
        for port in self.ports_states.keys(){
            ports.push(*port);
        }
        ports
    }

    async fn update_best(&mut self, bpdu: BPDU, port: u32){
        let default = (self.bpdu.clone(), 0);
        let (previous_best, cost) = self.ports.get(&self.root_port).unwrap_or(&default);
        
        let previous_best_distance_added = BPDU{root: previous_best.root, distance: previous_best.distance + cost, switch: previous_best.switch, port: previous_best.port};
        // if we received an update for the previous root port, recompute always the best bpdu
        // else, check if it is better than the previous root port
        let update = port == self.root_port || previous_best_distance_added > bpdu; 
        if update{
            self.bpdu = BPDU{root: bpdu.root, distance: bpdu.distance, switch: self.id, port: 0};
            self.root_port = port;
            self.logger.log(Source::SPT, format!("Updated BPDU of switch {} to {} and port {} became new root", self.name, self.bpdu.to_string(), port)).await;
            for port in self.get_ports(){
                self.update_state_port(port).await;
            }
        }
    }

    pub fn get_port_state(&self, port: u32) -> PortState{
        if self.root_port == port{
            PortState::Root
        }else{
            self.ports_states.get(&port).unwrap().clone()
        }
    }
}