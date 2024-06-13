use std::{collections::{hash_map::Entry, BinaryHeap, HashMap, HashSet}, net::Ipv4Addr, sync::Arc};

use tokio::sync::{mpsc::Sender, Mutex};

use crate::network::{logger::{Logger, Source}, messages::{ospf::OSPFMessage::{self, *}, Message}, router::RouterInfo};

#[derive(Ord, PartialEq, Eq, Hash, Clone)]
pub struct Node{
    distance: u32,
    ip: Ipv4Addr,
    port: u32
}

impl PartialOrd for Node{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

#[derive(Debug)]
pub struct OSPFState{
    pub topo: HashMap<Ipv4Addr, HashSet<(u32, Ipv4Addr)>>,
    pub direct_neighbors: HashSet<(u32, u32, Ipv4Addr)>,
    pub routing_table: HashMap<Ipv4Addr, (u32, u32)>,  // (port, distance)
    pub received_lsp: HashSet<(Ipv4Addr, u32)>,
    pub lsp_seq: u32,
    pub router_info: Arc<Mutex<RouterInfo>>,
    pub logger: Logger
}

impl OSPFState{
    pub fn new(ip: Ipv4Addr, logger: Logger, router_info: Arc<Mutex<RouterInfo>>) -> OSPFState{
        OSPFState{
            topo: HashMap::new(),
            direct_neighbors: HashSet::new(),
            routing_table: [(ip, (0, 0))].into_iter().collect(),
            received_lsp: HashSet::new(),
            lsp_seq: 0,
            router_info,
            logger
        }
    }

    pub fn get_port(&self, ip: Ipv4Addr) -> Option<&(u32, u32)>{
        self.routing_table.get(&ip)
    }

    pub async fn process_ospf(&mut self, ospf: OSPFMessage, port: u32){
        match ospf{
            Hello => self.send_hello_reply(port).await,
            LSP(from, seq, neighbors) => self.process_lsp(from, seq, neighbors).await,
            HelloReply(ip) => self.process_hello_reply(ip, port).await,
        }
    }

    pub async fn shortest_path(&mut self){
        let mut visited = HashSet::new();
        let mut pq = BinaryHeap::new();

        visited.insert(self.get_ip().await);
        for (cost, port, ip) in self.direct_neighbors.iter(){
            pq.push(Node{distance: *cost, ip: ip.clone(), port: *port});
        }

        while !pq.is_empty(){
            let p = pq.pop().unwrap();
            if visited.contains(&p.ip){
                continue;
            }
            self.routing_table.insert(p.ip, (p.port, p.distance));
            visited.insert(p.ip.clone());
            let neighs = self.topo.get(&p.ip);
            if let Some(n) = neighs{
                for (cost, neigh) in n{
                    pq.push(Node{distance: p.distance+cost, ip: *neigh, port: p.port});
                }
            }
        }
        self.logger.log(Source::OSPF, format!("Router {} has updated its routing table : {:?}", self.get_name().await, self.routing_table)).await;
    }

    pub async fn process_lsp(&mut self, from: Ipv4Addr, seq: u32, neighbors: HashSet<(u32, Ipv4Addr)>){
        if self.received_lsp.contains(&(from, seq)){
            return;
        }
        self.received_lsp.insert((from, seq));
        let values = match self.topo.entry(from) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        values.extend(neighbors.iter());
        self.shortest_path().await;

        self.send_lsp(OSPFMessage::LSP(from, seq, neighbors)).await; // flood
    }

    pub async fn process_hello_reply(&mut self, ip: Ipv4Addr, port: u32){
        if self.get_ip().await == ip{
            return;
        }
        let map = self.get_neighbors().await;
        let (_, cost) = map.get(&port).unwrap();
        if self.direct_neighbors.contains(&(*cost, port, ip)){
            return;
        }
        self.direct_neighbors.insert((*cost, port, ip));
        self.logger.log(Source::OSPF, format!("Router {} has neighbors : {:?}", self.get_name().await, self.direct_neighbors)).await;
        self.routing_table.insert(ip, (port, *cost));

        let values = match self.topo.entry(self.get_ip().await) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        values.insert((*cost, ip));
        
        self.logger.log(Source::OSPF, format!("Router {} received prefix {} from neighbor on port {}", self.get_name().await, ip, port)).await;
        let seq = self.lsp_seq;
        self.lsp_seq+=1;
        let mut neighs = HashSet::new();
        for (cost, _port, n) in self.direct_neighbors.iter(){
            neighs.insert((*cost, n.clone()));
        }
        let ip = self.get_ip().await;
        self.send_lsp(OSPFMessage::LSP(ip, seq, neighs)).await;
    }

    pub async fn send_lsp(&mut self, lsp: OSPFMessage){
        for (port, (sender, _)) in self.get_neighbors().await.iter() {
            self.logger.log(Source::OSPF, format!("Router {} sending {:?} on port {}", self.get_name().await, lsp, port)).await;
            sender.send(Message::OSPF(lsp.clone())).await.unwrap();
        }
    }

    pub async fn send_hello(&self){
        for (port, (sender, _)) in self.get_neighbors().await.iter() {
            let msg = Message::OSPF(Hello);
            self.logger.log(Source::OSPF, format!("Router {} sending Hello on port {}", self.get_name().await, port)).await;
            sender.send(msg).await.unwrap();
        }
    }

    pub async fn send_hello_reply(&self, port: u32){
        let map = self.get_neighbors().await;
        let (sender, _) = map.get(&port).unwrap();
        self.logger.log(Source::OSPF, format!("Router {} sending hello reply on port {}", self.get_name().await, port)).await;
        sender.send(Message::OSPF(OSPFMessage::HelloReply(self.get_ip().await))).await.expect("Failed to send Hello reply");
    }

    pub async fn get_ip(&self) -> Ipv4Addr{
        self.router_info.lock().await.ip
    }

    pub async fn get_name(&self) -> String{
        self.router_info.lock().await.name.clone()
    }

    pub async fn get_neighbors(&self) -> HashMap<u32, (Sender<Message>, u32)>{
        let mut map = HashMap::new();
        for (port, (_, sender, cost)) in self.router_info.lock().await.neighbors.iter(){
            map.insert(*port, (sender.clone(), *cost));
        }
        map
    }
}