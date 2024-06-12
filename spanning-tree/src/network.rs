pub mod switch;
pub mod router;
pub mod communicators;
pub mod logger;
pub mod messages;
pub mod protocols;
use logger::{Logger, Source};
use protocols::bgp::BGPRoute;
use switch::PortState;
use tokio::sync::mpsc::channel;
use std::{collections::{BTreeMap, HashMap}, net::Ipv4Addr, vec};

use self::switch::Switch;
use self::router::Router;
use self::communicators::{SwitchCommunicator, RouterCommunicator};

#[derive(Debug)]
pub struct Network{
    switches: BTreeMap<String, SwitchCommunicator>,
    routers: BTreeMap<String, (RouterCommunicator, Ipv4Addr)>,
    links: Vec<(String, u32, String, u32, u32)>,
    logger: Logger
}

impl Network{
    pub fn new() -> Network{
        Network{switches: BTreeMap::new(), routers: BTreeMap::new(), links: vec![], logger: Logger::start()}
    }

    pub fn new_with_filters(filters: Vec<Source>) -> Network{
        Network{switches: BTreeMap::new(), routers: BTreeMap::new(), links: vec![], logger: Logger::start_with_filters(filters)}
    }

    pub fn add_switch(&mut self, name: String, id: u32){
        let communicator = Switch::start(name.clone(), id, self.logger.clone());
        self.switches.insert(name, communicator);
    }

    pub fn add_router(&mut self, name: String, id: u32, router_as: u32){
        let communicator = Router::start(name.clone(), id, router_as, self.logger.clone());
        self.routers.insert(name, (communicator, Ipv4Addr::new(10, 0, router_as as u8, id as u8)));
    }

    pub async fn add_peer_link(&mut self, device1: String, port1: u32, device2: String, port2: u32, med: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);

        let (r1, ip1) = self.routers.get(&device1).expect(format!("Unknown device {}", device1).as_str());
        let (r2, ip2) = self.routers.get(&device2).expect(format!("Unknown device {}", device1).as_str());
        r1.add_peer_link(rx1, tx2, port1, med, *ip2).await;
        r2.add_peer_link(rx2, tx1, port2, med, *ip1).await;
    }

    pub async fn add_provider_customer_link(&mut self, provider: String, port1: u32, customer: String, port2: u32, med: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);

        let (provider, ip_provider) = self.routers.get(&provider).expect(format!("Unknown device {}", provider).as_str());
        let (customer, ip_customer) = self.routers.get(&customer).expect(format!("Unknown device {}", customer).as_str());

        provider.add_customer_link(rx1, tx2, port1, med, *ip_customer).await;
        customer.add_provider_link(rx2, tx1, port2, med, *ip_provider).await;
    }

    pub async fn add_link(&mut self, device1: String, port1: u32, device2: String, port2: u32, cost: u32){
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);
        match self.switches.get(&device1){
            Some(s) => s.add_link(rx1, tx2, port1, cost).await,
            None => match self.routers.get(&device1){
                Some((r, _)) => r.add_link(rx1, tx2, port1, cost).await,
                None => panic!("Missing device {}", device1)
            }
        };
        
        match self.switches.get(&device2){
            Some(s) => s.add_link(rx2, tx1, port2, cost).await,
            None => match self.routers.get(&device2){
                Some((r, _)) => r.add_link(rx2, tx1, port2, cost).await,
                None => panic!("Missing device {}", device2)
            }
        };

        self.links.push((device1, port1, device2, port2, cost));
    }

    pub async fn ping(&self, from: String, to: Ipv4Addr){
        let src = &self.routers.get(&from).expect("Unknown router").0;

        src.ping(to).await;
    }

    pub async fn announce_prefix(&self, router: String){
        let router = &self.routers.get(&router).expect("Unknown router").0;

        router.announce_prefix().await;
    }

    pub async fn get_routing_table(&self, router: String) -> HashMap<Ipv4Addr, (u32, u32)>{
        let src = &self.routers.get(&router).expect("Unknown router").0;

        src.get_routing_table().await.expect("Failed to retrieve routing table")
    }

    pub async fn get_bgp_routes(&self, router: String) -> HashMap<Ipv4Addr, Vec<BGPRoute>>{
        let src = &self.routers.get(&router).expect("Unknown router").0;

        src.get_bgp_routes().await.expect("Failed to retrieve bgp routes")
    }

    pub async fn quit(self){
        for (_, communicator) in self.switches{
            communicator.quit().await;
        }

        for (_, (communicator, _)) in self.routers{
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

    pub async fn print_routing_table(&self, router: String){
        let routing_tbale = self.get_routing_table(router.clone()).await;

        println!("{}", router);

        for (ip, (port, distance)) in routing_tbale{
            println!("  {}: port={}, distance={}", ip, port, distance);
        }
    }

    pub async fn print_routing_tables(&self){
        for router in self.routers.keys(){
            self.print_routing_table(router.clone()).await;
        }
    }

    pub async fn print_bgp_table(&self, router: String){
        let bgp_table = self.get_bgp_routes(router.clone()).await;

        println!("{}", router);

        for (prefix, routes) in bgp_table{
            println!("  {}", prefix);
            for route in routes{
                println!("    {}", route)
            }
        }
    }

    pub async fn print_bgp_tables(&self){
        for router in self.routers.keys(){
            self.print_bgp_table(router.clone()).await;
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
    use protocols::bgp::RouteSource;

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_spanning_tree() {
        for _ in 0..10{
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_ospf() {
        for _ in 0..10{
            let mut network = Network::new_with_filters(vec![Source::Ping]);
            network.add_router("r1".into(), 1, 1);
            network.add_router("r2".into(), 2, 1);
            network.add_router("r3".into(), 3, 1);
            network.add_router("r4".into(), 4, 1);
        
            network.add_link("r1".into(), 1, "r2".into(), 1, 1).await;
            network.add_link("r1".into(), 2, "r3".into(), 1, 1).await;
            network.add_link("r3".into(), 3, "r4".into(), 1, 1).await;
            network.add_link("r2".into(), 2, "r3".into(), 2, 1).await;
        
            // wait for convergence
            thread::sleep(Duration::from_millis(250));

            assert_eq!(network.get_routing_table("r1".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (0, 0)), 
                (Ipv4Addr::new(10, 0, 1, 2), (1, 1)), 
                (Ipv4Addr::new(10, 0, 1, 3), (2, 1)), 
                (Ipv4Addr::new(10, 0, 1, 4), (2, 2))
                ].into_iter().collect());

            assert_eq!(network.get_routing_table("r2".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (1, 1)), 
                (Ipv4Addr::new(10, 0, 1, 2), (0, 0)), 
                (Ipv4Addr::new(10, 0, 1, 3), (2, 1)), 
                (Ipv4Addr::new(10, 0, 1, 4), (2, 2))
                ].into_iter().collect());

            assert_eq!(network.get_routing_table("r3".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (1, 1)), 
                (Ipv4Addr::new(10, 0, 1, 2), (2, 1)), 
                (Ipv4Addr::new(10, 0, 1, 3), (0, 0)), 
                (Ipv4Addr::new(10, 0, 1, 4), (3, 1))
                ].into_iter().collect());

            assert_eq!(network.get_routing_table("r4".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (1, 2)), 
                (Ipv4Addr::new(10, 0, 1, 2), (1, 2)), 
                (Ipv4Addr::new(10, 0, 1, 3), (1, 1)), 
                (Ipv4Addr::new(10, 0, 1, 4), (0, 0))
                ].into_iter().collect());
        
            network.quit().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_mix_switches_routers() {
        for _ in 0..10{
            let mut network = Network::new_with_filters(vec![]);
            network.add_router("r1".into(), 1, 1);
            network.add_router("r2".into(), 2, 1);
            network.add_switch("s1".into(), 11);
            network.add_switch("s2".into(), 12);
            network.add_switch("s3".into(), 13);
            network.add_switch("s4".into(), 14);
        
            network.add_link("r1".into(), 1, "s1".into(), 1, 1).await;
            network.add_link("s1".into(), 2, "s2".into(), 1, 1).await;
            network.add_link("s2".into(), 2, "s3".into(), 1, 1).await;
            network.add_link("s4".into(), 1, "s3".into(), 3, 1).await;
            network.add_link("s4".into(), 2, "s1".into(), 3, 1).await;
            network.add_link("s3".into(), 2, "r2".into(), 1, 1).await;
        
            // wait for convergence
            thread::sleep(Duration::from_millis(250));
        
            assert_eq!(network.get_routing_table("r1".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (0, 0)), 
                (Ipv4Addr::new(10, 0, 1, 2), (1, 1))
                ].into_iter().collect());
        
            assert_eq!(network.get_routing_table("r2".into()).await, [
                (Ipv4Addr::new(10, 0, 1, 1), (1, 1)), 
                (Ipv4Addr::new(10, 0, 1, 2), (0, 0))
                ].into_iter().collect());
        
            thread::sleep(Duration::from_millis(250));
        
            network.quit().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_bgp(){
        for _ in 0..5{
            let mut network = Network::new_with_filters(vec![Source::BGP]);
            network.add_router("r1".into(), 1, 1);
            network.add_router("r2".into(), 2, 2);
            network.add_router("r3".into(), 3, 3);
            network.add_router("r4".into(), 4, 4);
        
            network.add_provider_customer_link("r2".into(), 1, "r1".into(), 1, 0).await;
            network.add_provider_customer_link("r2".into(), 2, "r4".into(), 1, 0).await;
            network.add_provider_customer_link("r4".into(), 3, "r3".into(), 1, 0).await;
        
            network.add_peer_link("r1".into(), 2, "r4".into(), 2, 0).await;
        
        
            network.announce_prefix("r1".into()).await;
        
            // wait for convergence
            thread::sleep(Duration::from_millis(1000));

            assert_eq!(network.get_bgp_routes("r2".into()).await, [
                ("10.0.1.1".parse().unwrap(), vec![
                    BGPRoute{
                        prefix: "10.0.1.1".parse().unwrap(), 
                        nexthop: "10.0.1.1".parse().unwrap(),
                        as_path: vec![1],
                        pref: 150,
                        med: 0,
                        router_id: 1,
                        source: RouteSource::EBGP
                    }])
                ].into_iter().collect());

            assert_eq!(network.get_bgp_routes("r3".into()).await, [
                ("10.0.1.1".parse().unwrap(), vec![
                    BGPRoute{
                        prefix: "10.0.1.1".parse().unwrap(), 
                        nexthop: "10.0.4.4".parse().unwrap(),
                        as_path: vec![4, 1],
                        pref: 50,
                        med: 0,
                        router_id: 4,
                        source: RouteSource::EBGP
                    }])
                ].into_iter().collect());

            assert_eq!(network.get_bgp_routes("r4".into()).await, [
                ("10.0.1.1".parse().unwrap(), vec![
                    BGPRoute{
                        prefix: "10.0.1.1".parse().unwrap(), 
                        nexthop: "10.0.1.1".parse().unwrap(),
                        as_path: vec![1],
                        pref: 100,
                        med: 0,
                        router_id: 1,
                        source: RouteSource::EBGP
                    },
                    BGPRoute{
                        prefix: "10.0.1.1".parse().unwrap(), 
                        nexthop: "10.0.2.2".parse().unwrap(),
                        as_path: vec![2, 1],
                        pref: 50,
                        med: 0,
                        router_id: 2,
                        source: RouteSource::EBGP
                    }])
                ].into_iter().collect());
        
            network.quit().await;        
        }
    }
}