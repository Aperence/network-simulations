pub mod switch;
use switch::PortState;
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};
use std::{collections::{BTreeMap, HashMap}, sync::Arc};

use self::switch::Switch;

type MutableSwitch = Arc<Mutex<Switch>>;

#[derive(Debug)]
pub struct Network{
    switches: HashMap<String, MutableSwitch>,
    connections: Vec<(MutableSwitch, u32, MutableSwitch, u32, u32)>,
    converged: Receiver<()>,
    converged_sender: Sender<()>
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

impl Network{
    pub fn new() -> Network{
        let (tx, rx) = channel(1024);
        Network{switches: HashMap::new(), connections: vec![], converged: rx, converged_sender: tx}
    }

    pub fn add_switch(&mut self, name: String, id: u32){
        let s = Switch::new(name.clone(), id);
        self.switches.insert(name, Arc::new(Mutex::new(s)));
    }

    pub async fn add_link(&mut self, switch1: String, port1: u32, switch2: String, port2: u32, cost: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);
        let s1= Arc::clone(self.switches.get_mut(&switch1).unwrap());
        let s2 = Arc::clone(self.switches.get_mut(&switch2).unwrap());
        Switch::add_link(Arc::clone(&s1), port1, rx1, tx2, cost).await;
        Switch::add_link(Arc::clone(&s2), port2, rx2, tx1, cost).await;
        self.connections.push((s1, port1, s2, port2, cost))
    }

    pub async fn run(&self, verbose: bool, start_directly_from_root: bool){
        for (_, switch) in self.switches.iter(){
            let mut switch = switch.lock().await;
            switch.set_converged_sender(self.converged_sender.clone());
            if verbose{
                println!("Initial BPDU for switch {} : {}", switch.name, switch.bpdu.to_string());
            }
            if !start_directly_from_root{
                switch.send_bpdu(verbose).await;
            }
        }

        if start_directly_from_root{
            let mut switch_lowest_id: Option<&MutableSwitch> = None;
            for (_, switch) in self.switches.iter(){
                let switch_locked = switch.lock().await;
                if switch_lowest_id.is_none() || switch_lowest_id.unwrap().lock().await.id > switch_locked.id{
                    switch_lowest_id = Some(switch);
                }
            }
            switch_lowest_id.unwrap().lock().await.send_bpdu(verbose).await;
        }
    }

    pub async fn wait_end(&mut self){
        let mut count = self.switches.len()-1;
        while count > 0{
            match self.converged.recv().await{
                Some(_) => count-=1,
                None => (),
            }
        }
    }

    pub async fn get_port_states(&self) -> BTreeMap<String, BTreeMap<u32, PortState>>{
        let mut states = BTreeMap::new();
        for switch in self.switches.values(){
            let switch = switch.lock().await;
            let mut map = BTreeMap::new();
            let ports: Vec<&u32> = switch.ports_states.keys().collect();
            for port in ports{
                map.insert(*port, switch.get_port_state(*port));
            }
            states.insert(switch.name.clone(), map);
        }
        states
    }

    pub async fn print_switch_states(&self){
        let states = self.get_port_states().await;
        for (switch, ports) in states{
            println!("{}", switch);
            for (port, state) in ports{
                println!("  {}: {:?}", port, state);
            }
        }
    }

    pub async fn print_dot(&self){
        println!("graph {{\n  \
            graph [nodesep=\"2\", ranksep=\"1\"];\n  \
            splines=\"false\";\n  \
            node[shape = diamond];");
        for (s1, port1, s2, port2, cost) in self.connections.iter(){
            let s1 = s1.lock().await;
            let s2 = s2.lock().await;
            println!("  \"{}\" -- \"{}\" [headlabel=\" {} {}\", taillabel=\" {} {}\", label=\" {}\"];", 
                s1.name, s2.name, port1, s1.get_port_state(port1.clone()).to_string(), port2, s2.get_port_state(port2.clone()).to_string(), cost);
        }
        println!("}}");
    }
}