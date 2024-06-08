use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::{mpsc::{error::TryRecvError, Receiver, Sender}, Mutex};

const CONVERGE_DELAY_MILLIS: u128 = 250;

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

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct BPDU{
    root: u32,
    distance: u32,
    switch: u32,
    port: u32
}

impl ToString for BPDU{
    fn to_string(&self) -> String{
        format!("<{},{},{},{}>", self.root, self.distance, self.switch, self.port)
    }
}

#[derive(Debug)]
pub struct Switch{
    pub name: String,
    pub id: u32,
    pub neighbors: HashMap<u32, (Sender<BPDU>, u32)>,
    pub bpdu: BPDU,
    pub root_port: u32,
    pub ports: HashMap<u32, (BPDU, u32)>,
    pub ports_states: HashMap<u32, PortState>,
    pub converged_sender: Option<Sender<()>>,
    pub prev_time: Option<SystemTime>
}

impl ToString for Switch{
    fn to_string(&self) -> String{
        format!("Switch {}", self.name)
    }
}

impl Switch{
    pub fn new(name: String, id: u32) -> Switch{
        Switch{name, id, neighbors: HashMap::new(), ports: HashMap::new(), ports_states: HashMap::new(), root_port: 0, bpdu: BPDU{root: id, distance: 0, switch: id, port: 0}, converged_sender: None, prev_time: None}
    }

    pub fn set_converged_sender(&mut self, sender: Sender<()>){
        self.converged_sender = Some(sender);
        self.prev_time = Some(SystemTime::now());
    }

    pub async fn add_link(switch: Arc<Mutex<Switch>>, port: u32, receiver: Receiver<BPDU>, other_sender: Sender<BPDU>, cost: u32){
        let cloned = switch.clone();
        switch.lock().await.neighbors.insert(port, (other_sender, cost));
        switch.lock().await.ports_states.insert(port, PortState::Designated);
        tokio::spawn(async move {
            Self::receive_loop(cloned, receiver, port, cost).await;
        });
    }

    pub async fn receive_loop(switch: Arc<Mutex<Switch>>, mut receiver: Receiver<BPDU>, port: u32, cost: u32){
        let mut done = false;
        while !done{
            match receiver.try_recv(){
                Ok(bpdu) => {
                    let mut switch = switch.lock().await;
                    switch.prev_time = Some(SystemTime::now());
                    switch.receive_bpdu(bpdu, port, cost, true).await;
                },
                Err(TryRecvError::Disconnected) => done = true,
                Err(TryRecvError::Empty) => {
                    let switch = switch.lock().await;
                    if let Some(time) = switch.prev_time{
                        match time.elapsed(){
                            Ok(t) => done = t.as_millis() > CONVERGE_DELAY_MILLIS,
                            Err(_) => (),
                        }
                    }
                }
            }
        }
        let mut switch = switch.lock().await;
        switch.neighbors.clear(); // drop senders
        let sender = switch.converged_sender.clone().unwrap();
        sender.send(()).await.unwrap();
    }

    pub async fn receive_bpdu(&mut self, bpdu: BPDU, port: u32, distance: u32, verbose: bool){
        if verbose{
            println!("Switch {} received BPDU {} on port {}", self.name, bpdu.to_string(), port);
        }
        let prev = self.ports.get(&port);
        if let Some((prev_bpdu, _)) = prev{
            if prev_bpdu < &bpdu{
                return;
            }
        }
        self.ports.insert(port, (bpdu.clone(), distance));
        self.update_best(BPDU{root: bpdu.root, distance: bpdu.distance+distance, switch: bpdu.switch, port: bpdu.port}, port, verbose);
        self.update_state_port(port, verbose);
        // updated root, resend my bpdu to all neighbors
        if self.root_port == port{
            self.send_bpdu(verbose).await;
        }
    }

    fn update_state_port(&mut self, port: u32, verbose: bool){
        let bpdu = self.ports.get(&port);
        if bpdu.is_none(){
            return;
        }
        let (bpdu, _) = bpdu.unwrap();
        if port == self.root_port{
            self.ports_states.insert(port, PortState::Root);

        }else if bpdu < &self.bpdu{
            if verbose{
                println!("BPDU received ({}) by {} on port {} was better than self bpdu ({}), port {} becomes blocked", bpdu.to_string(), self.name, port, self.bpdu.to_string(), port)
            }
            self.ports_states.insert(port, PortState::Blocked);
        }else{
            if verbose{
                println!("BPDU received ({}) by {} on port {} was worse than self bpdu ({}), port {} becomes designated", bpdu.to_string(), self.name, port, self.bpdu.to_string(), port)
            }
            self.ports_states.insert(port, PortState::Designated);
        }
    }

    pub async fn send_bpdu(&self, verbose: bool){
        for (port, (sender, _)) in self.neighbors.iter() {
            if self.get_port_state(*port) != PortState::Designated{
                // either we can't send a bpdu on this port, or it generated a cycle for rust borrows, no point to continue
                continue;
            }
            let bpdu = BPDU{root: self.bpdu.root, distance: self.bpdu.distance, switch: self.id, port: *port};
            if verbose{
                println!("Switch {} sending BPDU {} on port {}", self.name, bpdu.to_string(), port)
            }
            sender.send(bpdu).await.unwrap();
        }
    }

    fn get_ports(&self) -> Vec<u32>{
        let mut ports = vec![];
        for port in self.ports_states.keys(){
            ports.push(*port);
        }
        ports
    }

    fn update_best(&mut self, bpdu: BPDU, port: u32, verbose: bool){
        let previous_best = self.ports.get(&self.root_port);
        let better;
        match previous_best {
            None => better = self.bpdu > bpdu,
            Some((previous_bpdu, cost)) => {
                better= BPDU{root: previous_bpdu.root, distance: previous_bpdu.distance + cost, switch: previous_bpdu.switch, port: previous_bpdu.port} > bpdu && self.bpdu > bpdu
            }
        }
        if better{
            self.bpdu = BPDU{root: bpdu.root, distance: bpdu.distance, switch: self.id, port: 0};
            self.root_port = port;
            if verbose{
                println!("Updated BPDU of switch {} to {} and port {} became new root", self.name, self.bpdu.to_string(), port);
            }
            for port in self.get_ports(){
                self.update_state_port(port.clone(), verbose);
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