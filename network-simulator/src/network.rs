pub mod communicators;
pub mod logger;
pub mod messages;
pub mod protocols;
pub mod ip_trie;
pub mod router;
pub mod switch;
use ip_trie::IPPrefix;
use logger::{Logger, Source};
use protocols::bgp::BGPRoute;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::Ipv4Addr,
    vec,
};
use switch::PortState;
use tokio::sync::mpsc::channel;

use self::communicators::{RouterCommunicator, SwitchCommunicator};
use self::router::Router;
use self::switch::Switch;

#[derive(Debug)]
pub struct Network {
    switches: BTreeMap<String, SwitchCommunicator>,
    routers: BTreeMap<String, (RouterCommunicator, Ipv4Addr)>,
    used_port: BTreeMap<String, HashSet<u32>>,
    links: Vec<(String, u32, String, u32, u32)>,
    router_as: HashMap<u32, Vec<String>>,
    logger: Logger,
}

impl Network {
    pub fn new() -> Network {
        Network {
            switches: BTreeMap::new(),
            routers: BTreeMap::new(),
            used_port: BTreeMap::new(),
            links: vec![],
            router_as: HashMap::new(),
            logger: Logger::start(),
        }
    }

    pub fn new_with_filters(filters: Vec<Source>) -> Network {
        Network {
            switches: BTreeMap::new(),
            routers: BTreeMap::new(),
            used_port: BTreeMap::new(),
            links: vec![],
            router_as: HashMap::new(),
            logger: Logger::start_with_filters(filters),
        }
    }

    pub fn add_switch(&mut self, name: &str, id: u32) {
        let communicator = Switch::start(name.to_string(), id, self.logger.clone());
        self.switches.insert(name.to_string(), communicator);
        self.used_port.insert(name.to_string(), HashSet::new());
    }

    pub fn add_router(&mut self, name: &str, id: u32, router_as: u32) {
        let communicator = Router::start(name.to_string(), id, router_as, self.logger.clone());
        self.used_port.insert(name.to_string(), HashSet::new());
        self.routers.insert(
            name.to_string(),
            (
                communicator,
                Ipv4Addr::new(10, 0, router_as as u8, id as u8),
            ),
        );
        self.router_as.entry(router_as).or_insert(vec![]).push(name.to_string());
    }

    pub fn routers(&self) -> Vec<String>{
        self.routers.keys().map(|r| r.clone()).into_iter().collect()
    }

    pub fn check_port_not_used(&mut self, device: &str, port: u32){
        let ports = self.used_port.get_mut(device).unwrap();
        if ports.contains(&port){
            panic!("Port {} is already used for device {}", port, device);
        }else{
            ports.insert(port);
        }
    }

    pub async fn add_peer_link(
        &mut self,
        device1: &str,
        port1: u32,
        device2: &str,
        port2: u32,
        med: u32,
    ) {
        self.check_port_not_used(device1, port1);
        self.check_port_not_used(device2, port2);
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);

        let (r1, ip1) = self
            .routers
            .get(&device1.to_string())
            .expect(format!("Unknown device {}", device1).as_str());
        let (r2, ip2) = self
            .routers
            .get(&device2.to_string())
            .expect(format!("Unknown device {}", device1).as_str());
        r1.add_peer_link(rx1, tx2, port1, med, *ip2).await;
        r2.add_peer_link(rx2, tx1, port2, med, *ip1).await;
    }

    pub async fn add_provider_customer_link(
        &mut self,
        provider: &str,
        port1: u32,
        customer: &str,
        port2: u32,
        med: u32,
    ) {
        self.check_port_not_used(provider, port1);
        self.check_port_not_used(customer, port2);
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);

        let (provider, ip_provider) = self
            .routers
            .get(&provider.to_string())
            .expect(format!("Unknown device {}", provider).as_str());
        let (customer, ip_customer) = self
            .routers
            .get(&customer.to_string())
            .expect(format!("Unknown device {}", customer).as_str());

        provider
            .add_customer_link(rx1, tx2, port1, med, *ip_customer)
            .await;
        customer
            .add_provider_link(rx2, tx1, port2, med, *ip_provider)
            .await;
    }

    pub async fn add_link(
        &mut self,
        device1: &str,
        port1: u32,
        device2: &str,
        port2: u32,
        cost: u32,
    ) {
        self.check_port_not_used(device1, port1);
        self.check_port_not_used(device2, port2);
        let (tx1, rx1) = channel(1024);
        let (tx2, rx2) = channel(1024);
        match self.switches.get(&device1.to_string()) {
            Some(s) => s.add_link(rx1, tx2, port1, cost).await,
            None => match self.routers.get(&device1.to_string()) {
                Some((r, _)) => r.add_link(rx1, tx2, port1, cost).await,
                None => panic!("Missing device {}", device1),
            },
        };

        match self.switches.get(&device2.to_string()) {
            Some(s) => s.add_link(rx2, tx1, port2, cost).await,
            None => match self.routers.get(&device2.to_string()) {
                Some((r, _)) => r.add_link(rx2, tx1, port2, cost).await,
                None => panic!("Missing device {}", device2),
            },
        };

        self.links.push((device1.to_string(), port1, device2.to_string(), port2, cost));
    }

    pub async fn add_ibgp_connection(
        &mut self,
        device1: &str,
        device2: &str,
    ) {
        let (d1, ip1) = self
            .routers
            .get(&device1.to_string())
            .expect(format!("Unknown device {}", device1).as_str());
        let (d2, ip2) = self
            .routers
            .get(&device2.to_string())
            .expect(format!("Unknown device {}", device2).as_str());

        d1.add_ibgp_connection(*ip2).await;
        d2.add_ibgp_connection(*ip1).await;
    }

    pub async fn ping(&self, from: &str, to: Ipv4Addr) {
        let src = &self.routers.get(&from.to_string()).expect("Unknown router").0;

        src.ping(to).await;
    }

    pub async fn announce_prefix(&self, router: &str) {
        let router = &self.routers.get(router).expect("Unknown router").0;

        router.announce_prefix().await;
    }

    pub async fn announce_prefix_as(&self, announcing_as: u32) {
        for router in self.router_as.get(&announcing_as).unwrap(){
            self.announce_prefix(router).await;
        }
    }

    pub async fn get_routing_table(&self, router: &str) -> HashMap<IPPrefix, (u32, u32)> {
        let src = &self.routers.get(&router.to_string()).expect("Unknown router").0;

        src.get_routing_table()
            .await
            .expect("Failed to retrieve routing table")
    }

    pub async fn get_bgp_routes(
        &self,
        router: &str,
    ) -> HashMap<IPPrefix, (Option<BGPRoute>, HashSet<BGPRoute>)> {
        let src = &self.routers.get(&router.to_string()).expect("Unknown router").0;

        src.get_bgp_routes()
            .await
            .expect("Failed to retrieve bgp routes")
    }

    pub async fn quit(self) {
        for (_, communicator) in self.switches {
            communicator.quit().await;
        }

        for (_, (communicator, _)) in self.routers {
            communicator.quit().await;
        }
    }

    pub async fn get_port_states(&self) -> BTreeMap<String, BTreeMap<u32, PortState>> {
        let mut states = BTreeMap::new();
        for (switch, communicator) in self.switches.iter() {
            let ports_states = communicator
                .get_port_state()
                .await
                .unwrap_or_else(|_| panic!("Failed to get port states of {}", switch));
            states.insert(switch.clone(), ports_states);
        }
        states
    }

    pub async fn print_switch_states(&self) {
        let states = self.get_port_states().await;
        for (switch, ports) in states {
            println!("{}", switch);
            for (port, state) in ports {
                println!("  {}: {:?}", port, state);
            }
        }
    }

    pub async fn print_routing_table(&self, router: &str) {
        let routing_tbale = self.get_routing_table(router).await;

        println!("{}", router);

        for (ip, (port, distance)) in routing_tbale {
            println!("  {}: port={}, distance={}", ip, port, distance);
        }
    }

    pub async fn print_routing_tables(&self) {
        for router in self.routers.keys() {
            self.print_routing_table(router).await;
        }
    }

    pub async fn print_bgp_table(&self, router: &str) {
        let bgp_table = self.get_bgp_routes(router).await;

        println!("{}", router);

        for (prefix, (best_route, routes)) in bgp_table {
            println!("  {}", prefix);
            for route in routes {
                if Some(route.clone()) == best_route {
                    println!("   *{}", route)
                } else {
                    println!("    {}", route)
                }
            }
        }
    }

    pub async fn print_bgp_tables(&self) {
        for router in self.routers.keys() {
            self.print_bgp_table(router).await;
        }
    }

    pub async fn print_dot(&self) {
        let states = self.get_port_states().await;
        println!(
            "graph {{\n  \
            graph [nodesep=\"2\", ranksep=\"1\"];\n  \
            splines=\"false\";\n  \
            node[shape = diamond];"
        );
        for (s1, p1, s2, p2, cost) in self.links.iter() {
            println!(
                "  \"{}\" -- \"{}\" [headlabel=\" {} {}\", taillabel=\" {} {}\", label=\" {}\"];",
                s1,
                s2,
                p1,
                states.get(s1).unwrap().get(p1).unwrap().to_string(),
                p2,
                states.get(s2).unwrap().get(p2).unwrap().to_string(),
                cost
            );
        }
        println!("}}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocols::bgp::RouteSource;
    use std::thread;
    use std::time::Duration;
    use PortState::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test_spanning_tree() {
        for _ in 0..10 {
            let mut network = Network::new();
            network.add_switch("s1", 1);
            network.add_switch("s2", 2);
            network.add_switch("s3", 3);
            network.add_switch("s4", 4);
            network.add_switch("s6", 6);
            network.add_switch("s9", 9);

            network.add_link("s1", 1, "s2", 1, 1).await;
            network.add_link("s1", 2, "s4", 1, 1).await;
            network.add_link("s2", 2, "s9", 1, 1).await;
            network.add_link("s4", 2, "s9", 2, 1).await;
            network.add_link("s4", 3, "s3", 1, 1).await;
            network.add_link("s9", 3, "s3", 2, 1).await;
            network.add_link("s9", 4, "s6", 1, 1).await;
            network.add_link("s3", 3, "s6", 2, 1).await;

            // wait for convergence
            thread::sleep(Duration::from_millis(250));

            let switch_states = network.get_port_states().await;

            let mut expected: BTreeMap<String, BTreeMap<u32, PortState>> = BTreeMap::new();
            expected.insert(
                "s1".into(),
                [(1, Designated), (2, Designated)].into_iter().collect(),
            );
            expected.insert(
                "s2".into(),
                [(1, Root), (2, Designated)].into_iter().collect(),
            );
            expected.insert(
                "s3".into(),
                [(1, Root), (2, Designated), (3, Designated)]
                    .into_iter()
                    .collect(),
            );
            expected.insert(
                "s4".into(),
                [(1, Root), (2, Designated), (3, Designated)]
                    .into_iter()
                    .collect(),
            );
            expected.insert("s6".into(), [(1, Blocked), (2, Root)].into_iter().collect());
            expected.insert(
                "s9".into(),
                [(1, Root), (2, Blocked), (3, Blocked), (4, Designated)]
                    .into_iter()
                    .collect(),
            );

            assert_eq!(expected, switch_states);

            network.quit().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_ospf() {
        for _ in 0..10 {
            let mut network = Network::new_with_filters(vec![Source::Ping]);
            network.add_router("r1", 1, 1);
            network.add_router("r2", 2, 1);
            network.add_router("r3", 3, 1);
            network.add_router("r4", 4, 1);

            network.add_link("r1", 1, "r2", 1, 1).await;
            network.add_link("r1", 2, "r3", 1, 1).await;
            network.add_link("r3", 3, "r4", 1, 1).await;
            network.add_link("r2", 2, "r3", 2, 1).await;

            // wait for convergence
            thread::sleep(Duration::from_millis(250));

            assert_eq!(
                network.get_routing_table("r1").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (0, 0)),
                    ("10.0.1.2/32".parse().unwrap(), (1, 1)),
                    ("10.0.1.3/32".parse().unwrap(), (2, 1)),
                    ("10.0.1.4/32".parse().unwrap(), (2, 2))
                ]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_routing_table("r2").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (1, 1)),
                    ("10.0.1.2/32".parse().unwrap(), (0, 0)),
                    ("10.0.1.3/32".parse().unwrap(), (2, 1)),
                    ("10.0.1.4/32".parse().unwrap(), (2, 2))
                ]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_routing_table("r3").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (1, 1)),
                    ("10.0.1.2/32".parse().unwrap(), (2, 1)),
                    ("10.0.1.3/32".parse().unwrap(), (0, 0)),
                    ("10.0.1.4/32".parse().unwrap(), (3, 1))
                ]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_routing_table("r4").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (1, 2)),
                    ("10.0.1.2/32".parse().unwrap(), (1, 2)),
                    ("10.0.1.3/32".parse().unwrap(), (1, 1)),
                    ("10.0.1.4/32".parse().unwrap(), (0, 0))
                ]
                .into_iter()
                .collect()
            );

            network.quit().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test_mix_switches_routers() {
        for _ in 0..10 {
            let mut network = Network::new_with_filters(vec![]);
            network.add_router("r1", 1, 1);
            network.add_router("r2", 2, 1);
            network.add_switch("s1", 11);
            network.add_switch("s2", 12);
            network.add_switch("s3", 13);
            network.add_switch("s4", 14);

            network.add_link("r1", 1, "s1", 1, 1).await;
            network.add_link("s1", 2, "s2", 1, 1).await;
            network.add_link("s2", 2, "s3", 1, 1).await;
            network.add_link("s4", 1, "s3", 3, 1).await;
            network.add_link("s4", 2, "s1", 3, 1).await;
            network.add_link("s3", 2, "r2", 1, 1).await;

            // wait for convergence
            thread::sleep(Duration::from_millis(250));

            assert_eq!(
                network.get_routing_table("r1").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (0, 0)),
                    ("10.0.1.2/32".parse().unwrap(), (1, 1))
                ]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_routing_table("r2").await,
                [
                    ("10.0.1.1/32".parse().unwrap(), (1, 1)),
                    ("10.0.1.2/32".parse().unwrap(), (0, 0))
                ]
                .into_iter()
                .collect()
            );

            thread::sleep(Duration::from_millis(250));

            network.quit().await;
        }
    }

    
    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    async fn test_bgp() {
        for _ in 0..5 {
            let mut network = Network::new_with_filters(vec![Source::BGP]);
            network.add_router("r1", 1, 1);
            network.add_router("r2", 2, 2);
            network.add_router("r3", 3, 3);
            network.add_router("r4", 4, 4);

            network
                .add_provider_customer_link("r2", 1, "r1", 1, 0)
                .await;
            network
                .add_provider_customer_link("r2", 2, "r4", 1, 0)
                .await;
            network
                .add_provider_customer_link("r4", 3, "r3", 1, 0)
                .await;

            network
                .add_peer_link("r1", 2, "r4", 2, 0)
                .await;

            network.announce_prefix("r1").await;

            // wait for convergence
            thread::sleep(Duration::from_millis(1000));

            assert_eq!(
                network.get_bgp_routes("r2").await,
                [(
                    "10.0.1.0/24".parse().unwrap(),
                    (
                        Some(BGPRoute {
                            prefix: "10.0.1.0/24".parse().unwrap(),
                            nexthop: "10.0.1.1".parse().unwrap(),
                            as_path: vec![1],
                            pref: 150,
                            med: 0,
                            router_id: 1,
                            source: RouteSource::EBGP
                        }),
                        [BGPRoute {
                            prefix: "10.0.1.0/24".parse().unwrap(),
                            nexthop: "10.0.1.1".parse().unwrap(),
                            as_path: vec![1],
                            pref: 150,
                            med: 0,
                            router_id: 1,
                            source: RouteSource::EBGP
                        }]
                        .into_iter()
                        .collect()
                    )
                )]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_bgp_routes("r3").await,
                [(
                    "10.0.1.0/24".parse().unwrap(),
                    (
                        Some(BGPRoute {
                            prefix: "10.0.1.0/24".parse().unwrap(),
                            nexthop: "10.0.4.4".parse().unwrap(),
                            as_path: vec![4, 1],
                            pref: 50,
                            med: 0,
                            router_id: 4,
                            source: RouteSource::EBGP
                        }),
                        [BGPRoute {
                            prefix: "10.0.1.0/24".parse().unwrap(),
                            nexthop: "10.0.4.4".parse().unwrap(),
                            as_path: vec![4, 1],
                            pref: 50,
                            med: 0,
                            router_id: 4,
                            source: RouteSource::EBGP
                        }]
                        .into_iter()
                        .collect()
                    )
                )]
                .into_iter()
                .collect()
            );

            assert_eq!(
                network.get_bgp_routes("r4").await,
                [(
                    "10.0.1.0/24".parse().unwrap(),
                    (
                        Some(BGPRoute {
                            prefix: "10.0.1.0/24".parse().unwrap(),
                            nexthop: "10.0.1.1".parse().unwrap(),
                            as_path: vec![1],
                            pref: 100,
                            med: 0,
                            router_id: 1,
                            source: RouteSource::EBGP
                        }),
                        [
                            BGPRoute {
                                prefix: "10.0.1.0/24".parse().unwrap(),
                                nexthop: "10.0.1.1".parse().unwrap(),
                                as_path: vec![1],
                                pref: 100,
                                med: 0,
                                router_id: 1,
                                source: RouteSource::EBGP
                            },
                            BGPRoute {
                                prefix: "10.0.1.0/24".parse().unwrap(),
                                nexthop: "10.0.2.2".parse().unwrap(),
                                as_path: vec![2, 1],
                                pref: 50,
                                med: 0,
                                router_id: 2,
                                source: RouteSource::EBGP
                            }
                        ]
                        .into_iter()
                        .collect()
                    )
                )]
                .into_iter()
                .collect()
            );

            network.quit().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    pub async fn test_bgp_complex() {
        let mut network = Network::new_with_filters(vec![Source::Ping, Source::BGP]);
        network.add_router("r1", 1, 1);
        network.add_router("r2", 2, 2);
        network.add_router("r3", 3, 3);
        network.add_router("r4", 4, 4);
        network.add_router("r5", 5, 5);
        network.add_router("r6", 6, 6);
        network.add_router("r7", 7, 7);
        network.add_router("r8", 8, 8);

        network
            .add_provider_customer_link("r3", 1, "r1", 1, 0)
            .await;
        network
            .add_provider_customer_link("r1", 2, "r2", 1, 0)
            .await;
        network
            .add_provider_customer_link("r4", 1, "r3", 3, 0)
            .await;
        network
            .add_provider_customer_link("r5", 1, "r2", 3, 0)
            .await;
        network
            .add_provider_customer_link("r7", 1, "r4", 3, 0)
            .await;
        network
            .add_provider_customer_link("r6", 2, "r7", 2, 0)
            .await;
        network
            .add_provider_customer_link("r8", 1, "r7", 3, 0)
            .await;

        network
            .add_peer_link("r2", 2, "r3", 2, 0)
            .await;
        network
            .add_peer_link("r4", 2, "r5", 2, 0)
            .await;
        network
            .add_peer_link("r5", 3, "r6", 1, 0)
            .await;
        network
            .add_peer_link("r6", 3, "r8", 2, 0)
            .await;

        network.announce_prefix("r2").await;

        // wait for convergence
        thread::sleep(Duration::from_millis(2000));

        let routes1 = [(
            "10.0.2.0/24".parse().unwrap(),
            (
                Some(BGPRoute {
                    prefix: "10.0.2.0/24".parse().unwrap(),
                    nexthop: "10.0.2.2".parse().unwrap(),
                    as_path: vec![2],
                    pref: 150,
                    med: 0,
                    router_id: 2,
                    source: RouteSource::EBGP,
                }),
                [BGPRoute {
                    prefix: "10.0.2.0/24".parse().unwrap(),
                    nexthop: "10.0.2.2".parse().unwrap(),
                    as_path: vec![2],
                    pref: 150,
                    med: 0,
                    router_id: 2,
                    source: RouteSource::EBGP,
                }]
                .into_iter()
                .collect(),
            ),
        )]
            .into_iter()
            .collect();

        assert_eq!(network.get_bgp_routes("r1").await, routes1);
        network.quit().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    async fn test_ibgp(){
        for _ in 0..5{
            let mut network = Network::new_with_filters(vec![Source::BGP, Source::Ping]);
            network.add_router("r1", 1, 1);
            network.add_router("r2", 2, 1);
            network.add_router("r3", 3, 1);
            network.add_router("r4", 4, 2);
            network.add_router("r5", 5, 3);
        
            network
                .add_provider_customer_link("r4", 1, "r1", 1, 0)
                .await;
        
            network
                .add_provider_customer_link("r3", 3, "r5", 3, 0)
                .await;
        
            network
                .add_link("r1", 2, "r2", 1, 0)
                .await;
            network
                .add_link("r2", 2, "r3", 1, 0)
                .await;
            network
                .add_link("r1", 3, "r3", 2, 0)
                .await;
        
            let routers = ["r1", "r2", "r3"];
            for i in 0..routers.len(){
                for j in i+1..routers.len(){
                    network.add_ibgp_connection(routers[i].into(), routers[j].into()).await;
                }
            }
        
            // wait for convergence
            thread::sleep(Duration::from_millis(1000));
        
            network.announce_prefix("r4").await;
            network.announce_prefix("r5").await;
        
            thread::sleep(Duration::from_millis(1000));
        
            let bgp_table = network.get_bgp_routes("r2").await;
            let mut expected_table = HashMap::new();
            expected_table.insert("10.0.2.0/24".parse().unwrap(), (Some(BGPRoute{
                prefix: "10.0.2.0/24".parse().unwrap(),
                nexthop: "10.0.1.1".parse().unwrap(),
                as_path: vec![2],
                pref: 50,
                med: 0,
                router_id: 1,
                source: RouteSource::IBGP,
            }), [BGPRoute{
                prefix: "10.0.2.0/24".parse().unwrap(),
                nexthop: "10.0.1.1".parse().unwrap(),
                as_path: vec![2],
                pref: 50,
                med: 0,
                router_id: 1,
                source: RouteSource::IBGP,
            }].into_iter().collect()));

            expected_table.insert("10.0.3.0/24".parse().unwrap(), (Some(BGPRoute{
                prefix: "10.0.3.0/24".parse().unwrap(),
                nexthop: "10.0.1.3".parse().unwrap(),
                as_path: vec![3],
                pref: 150,
                med: 0,
                router_id: 3,
                source: RouteSource::IBGP,
            }), [BGPRoute{
                prefix: "10.0.3.0/24".parse().unwrap(),
                nexthop: "10.0.1.3".parse().unwrap(),
                as_path: vec![3],
                pref: 150,
                med: 0,
                router_id: 3,
                source: RouteSource::IBGP,
            }].into_iter().collect()));
            assert_eq!(bgp_table, expected_table);

        
            network.quit().await;
        }
    }
}
