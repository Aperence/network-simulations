use std::{cell::RefCell, collections::HashMap, rc::Rc};

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
    pub neighbors: HashMap<u32, (Rc<RefCell<Switch>>, u32, u32)>,  // (port, (switch, other_port, cost))
    pub bpdu: BPDU,
    pub root_port: u32,
    pub ports: HashMap<u32, BPDU>,
    pub ports_states: HashMap<u32, PortState>
}

impl ToString for Switch{
    fn to_string(&self) -> String{
        let neighbors: Vec<String> = self.neighbors.iter().map(|(k,(other, other_port, _dist))| format!("{}={}:{}", k, other.borrow().name, other_port)).collect();
        format!("Switch {}{{neighbors : [{}]}}", self.name, neighbors.join(", "))
    }
}

impl Switch{
    pub fn new(name: String, id: u32) -> Switch{
        Switch{name, id, neighbors: HashMap::new(), ports: HashMap::new(), ports_states: HashMap::new(), root_port: 0, bpdu: BPDU{root: id, distance: 0, switch: id, port: 0}}
    }

    pub fn add_link(&mut self, port: u32, other: Rc<RefCell<Switch>>, other_port: u32, cost: u32){
        self.neighbors.insert(port, (other, other_port, cost));
        self.ports_states.insert(port, PortState::Designated);
    }

    pub fn receive_bpdu(&mut self, bpdu: BPDU, port: u32, distance: u32, verbose: bool){
        if verbose{
            println!("Switch {} received BPDU {} on port {}", self.name, bpdu.to_string(), port);
        }
        let prev = self.ports.get(&port);
        if let Some(prev_bpdu) = prev{
            if prev_bpdu < &bpdu{
                return;
            }
        }
        self.ports.insert(port, bpdu.clone());
        let id = self.id;
        self.update_best(BPDU{root: bpdu.root, distance: bpdu.distance+distance, switch: id, port: 0}, port);
        if self.root_port == port{
            // updated root, resend my bpdu to all neighbors
            if verbose{
                println!("Updated own BPDU to {} and port {} became new root", self.bpdu.to_string(), port)
            }
            self.send_bpdu(verbose);
            return;
        }
        if bpdu < self.bpdu{
            if verbose{
                println!("BPDU received was better than self bpdu ({}), port {} becomes blocked", self.bpdu.to_string(), port)
            }
            self.ports_states.insert(port, PortState::Blocked);
        }else{
            if verbose{
                println!("BPDU received was worse than self bpdu ({}), port {} becomes designated", self.bpdu.to_string(), port)
            }
            self.ports_states.insert(port, PortState::Designated);
        }
    }

    pub fn send_bpdu(&self, verbose: bool){
        for (port, (other, other_port, cost)) in self.neighbors.iter() {
            let borrowed = other.try_borrow();
            if self.get_port_state(*port) != PortState::Designated || borrowed.is_err(){
                // either we can't send a bpdu on this port, or it generated a cycle for rust borrows, no point to continue
                continue;
            }
            let borrowed = borrowed.unwrap();
            if borrowed.id == self.bpdu.root{
                continue;
            }
            let bpdu = BPDU{root: self.bpdu.root, distance: self.bpdu.distance, switch: self.id, port: *port};
            if verbose{
                println!("Switch {} sending BPDU {} to {}", self.name, bpdu.to_string(), borrowed.name)
            }
            drop(borrowed);
            other.borrow_mut().receive_bpdu(bpdu, *other_port, *cost, verbose);
        }
    }

    fn update_best(&mut self, bpdu: BPDU, port: u32){
        if self.bpdu > bpdu{
            self.bpdu = bpdu;
            self.root_port = port;
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