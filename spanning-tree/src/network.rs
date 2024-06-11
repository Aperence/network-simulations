pub mod switch;
pub mod router;
pub mod communicators;
pub mod messages;
use switch::PortState;
use tokio::sync::mpsc::channel;
use std::{collections::{BTreeMap, HashMap}, vec};

use self::switch::Switch;
use self::router::Router;
use self::communicators::{SwitchCommunicator, RouterCommunicator};

#[derive(Debug)]
pub struct Network{
    switches: HashMap<String, SwitchCommunicator>,
    routers: HashMap<String, RouterCommunicator>,
    links: Vec<(String, u32, String, u32, u32)>,
}

impl Network{
    pub fn new() -> Network{
        Network{switches: HashMap::new(), routers: HashMap::new(), links: vec![]}
    }

    pub fn add_switch(&mut self, name: String, id: u32){
        let communicator = Switch::start(name.clone(), id);
        self.switches.insert(name, communicator);
    }

    pub fn add_router(&mut self, name: String, id: u32){
        let communicator = Router::start(name.clone(), id);
        self.routers.insert(name, communicator);
    }

    pub async fn add_link(&mut self, device1: String, port1: u32, device2: String, port2: u32, cost: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);
        match self.switches.get(&device1){
            Some(s) => s.add_link(rx1, tx2, port1, cost).await,
            None => match self.routers.get(&device1){
                Some(r) => r.add_link(rx1, tx2, port1, cost).await,
                None => panic!("Missing device {}", device1)
            }
        };
        
        match self.switches.get(&device2){
            Some(s) => s.add_link(rx2, tx1, port2, cost).await,
            None => match self.routers.get(&device2){
                Some(r) => r.add_link(rx2, tx1, port2, cost).await,
                None => panic!("Missing device {}", device2)
            }
        };

        self.links.push((device1, port1, device2, port2, cost));
    }

    pub async fn quit(self){
        for (_, communicator) in self.switches{
            communicator.quit().await;
        }

        for (_, communicator) in self.routers{
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


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use PortState::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_spanning_tree() {
        let mut network = Network::new();
        network.add_switch("s1".into(), 1);
        network.add_switch("s2".into(), 2);
        network.add_switch("s3".into(), 3);
        network.add_switch("s4".into(), 4);
        network.add_switch("s6".into(), 6);
        network.add_switch("s9".into(), 9);
    
        network.add_link("s1".into(), 1, "s2".into(), 1, 1).await;
        network.add_link("s1".into(), 2, "s4".into(), 1, 1).await;
        network.add_link("s2".into(), 2, "s9".into(), 1, 1).await;
        network.add_link("s4".into(), 2, "s9".into(), 2, 1).await;
        network.add_link("s4".into(), 3, "s3".into(), 1, 1).await;
        network.add_link("s9".into(), 3, "s3".into(), 2, 1).await;
        network.add_link("s9".into(), 4, "s6".into(), 1, 1).await;
        network.add_link("s3".into(), 3, "s6".into(), 2, 1).await;
    
        // wait for convergence
        thread::sleep(Duration::from_millis(250));

        let switch_states = network.get_port_states().await;
    
        let mut expected: BTreeMap<String, BTreeMap<u32, PortState>> = BTreeMap::new();
        expected.insert("s1".into(), [(1, Designated), (2, Designated)].into_iter().collect());
        expected.insert("s2".into(), [(1, Root), (2, Designated)].into_iter().collect());
        expected.insert("s3".into(), [(1, Root), (2, Designated), (3, Designated)].into_iter().collect());
        expected.insert("s4".into(), [(1, Root), (2, Designated), (3, Designated)].into_iter().collect());
        expected.insert("s6".into(), [(1, Blocked), (2, Root)].into_iter().collect());
        expected.insert("s9".into(), [(1, Root), (2, Blocked), (3, Blocked), (4, Designated)].into_iter().collect());

        assert_eq!(expected, switch_states);
    
        network.quit().await;
    }
}