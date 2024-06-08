pub mod switch;
use switch::PortState;
use tokio::sync::mpsc::channel;
use std::{collections::{BTreeMap, HashMap}, vec};

use self::switch::{Switch, SwitchCommunicator};

#[derive(Debug)]
pub struct Network{
    switches: HashMap<String, SwitchCommunicator>,
    links: Vec<(String, u32, String, u32, u32)>,
}

impl Network{
    pub fn new() -> Network{
        Network{switches: HashMap::new(), links: vec![]}
    }

    pub fn add_switch(&mut self, name: String, id: u32){
        let communicator = Switch::start(name.clone(), id);
        self.switches.insert(name, communicator);
    }

    pub async fn add_link(&mut self, switch1: String, port1: u32, switch2: String, port2: u32, cost: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);
        let s1 = self.switches.get(&switch1).unwrap_or_else(|| panic!("Missing switch {}", switch1));
        let s2 = self.switches.get(&switch2).unwrap_or_else(|| panic!("Missing switch {}", switch2));

        s1.add_link(rx1, tx2, port1, cost).await;
        s2.add_link(rx2, tx1, port2, cost).await;

        self.links.push((switch1, port1, switch2, port2, cost));
    }

    pub async fn quit(self){
        for (_, communicator) in self.switches{
            communicator.quit().await;
        }
    }

    pub async fn get_port_states(&self) -> BTreeMap<String, BTreeMap<u32, PortState>>{
        let mut states = BTreeMap::new();
        for (switch, communicator) in self.switches.iter(){
            let ports_states = communicator.get_port_state().await.unwrap_or_else(|_| panic!("Failed to get port states of {}", switch));
            states.insert(switch.clone(), ports_states);
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
        let states = self.get_port_states().await;
        println!("graph {{\n  \
            graph [nodesep=\"2\", ranksep=\"1\"];\n  \
            splines=\"false\";\n  \
            node[shape = diamond];");
        for (s1, p1, s2, p2, cost) in self.links.iter(){
            println!("  \"{}\" -- \"{}\" [headlabel=\" {} {}\", taillabel=\" {} {}\", label=\" {}\"];", 
            s1, s2, p1, states.get(s1).unwrap().get(p1).unwrap().to_string(), p2, states.get(s2).unwrap().get(p2).unwrap().to_string(), cost);
        }
        println!("}}");
    }
}