pub mod switch;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use self::switch::Switch;

#[derive(Debug)]
pub struct Network{
    switches: HashMap<String, Rc<RefCell<Switch>>>
}

impl ToString for Network{
    fn to_string(&self) -> String{
        let neighbors: Vec<String> = self.switches.values().map(|switch| switch.borrow().to_string()).collect();
        format!("Network{{switches : [{}]}}", neighbors.join(", "))
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

impl Network{
    pub fn new() -> Network{
        Network{switches: HashMap::new()}
    }

    pub fn add_switch(&mut self, name: String, id: u32){
        let s = Switch::new(name.clone(), id);
        self.switches.insert(name, Rc::new(RefCell::new(s)));
    }

    pub fn add_link(&mut self, switch1: String, port1: u32, switch2: String, port2: u32, cost: u32){
        let s1 = Rc::clone(self.switches.get_mut(&switch1).unwrap());
        let s2 = Rc::clone(self.switches.get_mut(&switch2).unwrap());
        s1.borrow_mut().add_link(port1, Rc::clone(&s2), port2, cost);
        s2.borrow_mut().add_link(port2, Rc::clone(&s1), port1, cost);
    }

    pub fn run(&self, verbose: bool, start_directly_from_root: bool){
        if verbose{
            for (_, switch) in self.switches.iter(){
                println!("Initial BPDU for switch {} : {}", switch.borrow().name, switch.borrow().bpdu.to_string());
            }
        }
        if start_directly_from_root{
            let mut switch_lowest_id: Option<&Rc<RefCell<Switch>>> = None;
            for (_, switch) in self.switches.iter(){
                if switch_lowest_id.is_none() || switch_lowest_id.unwrap().borrow().id > switch.borrow().id{
                    switch_lowest_id = Some(switch);
                }
            }
            switch_lowest_id.unwrap().borrow().send_bpdu(verbose);
        }else{
            for (_, switch) in self.switches.iter(){
                switch.borrow().send_bpdu(verbose);
            }
        }
    }

    pub fn switches(&self) -> Vec<Rc<RefCell<Switch>>>{
        let mut ret : Vec<Rc<RefCell<Switch>>> = self.switches.iter().map(|(_, s)| s.clone()).collect();
        ret.sort_by(|s1, s2| s1.borrow().name.cmp(&s2.borrow().name));
        ret
    }

    pub fn print_switch_states(&self){
        for switch in self.switches(){
            let switch = switch.borrow();
            println!("{}", switch.name);
            let mut ports: Vec<&u32> = switch.ports_states.keys().collect();
            ports.sort();
            for port in ports{
                println!("  {}: {:?}", port, switch.get_port_state(*port));
            }
        }
    }

    pub fn print_dot(&self){
        println!("graph {{\n  \
            graph [nodesep=\"2\", ranksep=\"1\"];\n  \
            splines=\"false\";\n  \
            node[shape = diamond];");
        for switch in self.switches(){
            let switch = switch.borrow();
            let mut ports: Vec<&u32> = switch.ports_states.keys().collect();
            ports.sort();
            for port in ports{
                let (other, other_port, cost) = switch.neighbors.get(port).unwrap();
                let other = other.borrow();
                if other.name < switch.name{
                    continue;
                }
                println!("  \"{}\" -- \"{}\" [headlabel=\" {} {}\", taillabel=\" {} {}\", label=\" {}\"];", switch.name, other.name, port, switch.get_port_state(*port).to_string(), other_port, other.get_port_state(*other_port).to_string(), cost);
            }
        }
        println!("}}");
    }
}