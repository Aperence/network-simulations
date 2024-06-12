use std::{borrow::Borrow, collections::{hash_map::Entry, HashMap}, net::Ipv4Addr, sync::Arc};

use tokio::sync::Mutex;

use crate::network::{
    logger::{Logger, Source},
    messages::{bgp::BGPMessage, Message},
    router::RouterInfo,
};

use super::ospf::OSPFState;

#[derive(Debug, PartialEq, Clone)]
pub enum RouteSource{
    IBGP,
    EBGP
}

#[derive(Debug, PartialEq, Clone)]
pub struct BGPRoute{
    prefix: Ipv4Addr,
    nexthop: Ipv4Addr,
    as_path: Vec<u32>,
    pref: u32,
    med: u32,
    router_id: u32,
    source: RouteSource
}

#[derive(Debug)]
pub struct BGPState {
    pub router_info: Arc<Mutex<RouterInfo>>,
    pub igp_info: Arc<Mutex<OSPFState>>,
    pub logger: Logger,
    routes: HashMap<Ipv4Addr, Vec<BGPRoute>>
}

impl BGPState {
    pub fn new(router_info: Arc<Mutex<RouterInfo>>, igp_info: Arc<Mutex<OSPFState>>, logger: Logger) -> BGPState {
        BGPState {
            router_info,
            igp_info,
            logger,
            routes: HashMap::new()
        }
    }

    pub async fn process_bgp_message(&mut self, port:u32, message: BGPMessage) {
        match message {
            BGPMessage::Update(prefix, nexthop, as_path, med, router_id) => {
                self.process_update(port, prefix, nexthop, as_path, med, router_id).await
            }
            BGPMessage::Withdraw(prefix, nexthop, as_path) => {
                self.process_withdraw(port, prefix, nexthop, as_path).await
            }
        }
    }

    pub async fn process_update(
        &mut self,
        port: u32,
        prefix: Ipv4Addr,
        nexthop: Ipv4Addr,
        as_path: Vec<u32>,
        med: u32,
        router_id: u32
    ) {
        
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        let pref = info.bgp_links.get(&port).unwrap().2;
        let current_as = info.router_as;
        drop(info);
        if as_path.contains(&current_as){
            return;
        }
        self.logger.borrow().log(Source::BGP, format!("Router {} received bgp update on port {} for prefix {} with nexthop = {}, AS path = {:?}, med = {}", name, port, prefix, nexthop, as_path, med)).await;
        let route = BGPRoute{prefix, nexthop, as_path, pref, med, source: RouteSource::EBGP, router_id};

        let previous_best = self.decision_process(prefix).await;

        let routes = match self.routes.entry(prefix) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(vec![]),
        };

        routes.push(route);

        let best = self.decision_process(prefix).await;

        if previous_best != best{
            if let Some(previous_best_route) = previous_best{
                self.send_withdraw(previous_best_route.prefix, previous_best_route.nexthop, previous_best_route.as_path).await;
            }
            let best = best.unwrap();
            self.send_update(best.prefix, best.nexthop, best.as_path).await;
        }
    }

    pub async fn process_withdraw(&self, port: u32, prefix: Ipv4Addr, nexthop: Ipv4Addr, as_path: Vec<u32>) {
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        drop(info);
        self.logger.borrow().log(Source::BGP, format!("Router {} received bgp withdraw on port {} for prefix {} with nexthop = {}, AS path = {:?}", name, port, prefix, nexthop, as_path)).await;
    
    }

    pub async fn distance_nexthop(&self, nexthop: Ipv4Addr){
        let routing_table = &self.igp_info.lock().await.routing_table;
        match routing_table.get(&nexthop){
            Some((_, distance)) => *distance,
            None => u32::max_value(),
        };
    }

    pub async fn decision_process(&self, prefix: Ipv4Addr) -> Option<BGPRoute>{
        let routes = self.routes.get(&prefix);

        if routes.is_none(){
            return None;
        }

        let routes = routes.unwrap();

        let mut best_pref = 0;
        let mut best_path_len = usize::max_value();
        for route in routes{
            if best_pref != route.pref{
                if route.pref > best_pref{
                    best_pref = route.pref;
                    best_path_len = route.as_path.len();
                }
            }else{
                best_path_len = usize::min(route.as_path.len(), best_path_len);
            }
        }

        let mut map = HashMap::new();
        for route in routes{
            if route.pref != best_pref || route.as_path.len() != best_path_len{
                continue;
            }
            println!("In");
            let map_entry = match map.entry(route.as_path[0]) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(vec![]),
            };

            if map_entry.len() == 0{
                map_entry.push(route);
            }else{
                if map_entry[0].med > route.med{
                    map_entry.clear();
                    map_entry.push(route);
                }else if map_entry[0].med == route.med{
                    map_entry.push(route);
                }
            }
        }

        let mut routes: Vec<&BGPRoute> = vec![];
        for route_vec in map.values(){
            routes.extend(route_vec.iter());
        }

        let mut best_route = routes[0];
        
        for route in routes{
            if best_route.source != route.source{
                if best_route.source == RouteSource::IBGP && route.source == RouteSource::EBGP{
                    best_route = route;
                }
            }
            else if best_route.source == RouteSource::IBGP && self.distance_nexthop(route.nexthop).await != self.distance_nexthop(best_route.nexthop).await{
                if self.distance_nexthop(route.nexthop).await < self.distance_nexthop(best_route.nexthop).await{
                    best_route = route;
                }
            }else{
                if route.router_id < best_route.router_id{
                    best_route = route;
                }
            }
        }

        Some(best_route.clone())
    }

    pub async fn send_update(&self, prefix: Ipv4Addr, nexthop: Ipv4Addr, mut as_path: Vec<u32>) {
        let info = self.router_info.lock().await;
        as_path.insert(0, info.router_as);
        for (_, (_, sender, _, med)) in info.bgp_links.iter() {
            let message = BGPMessage::Update(prefix, nexthop, as_path.clone(), *med, info.id);
            sender
                .send(Message::BGP(message))
                .await
                .expect("Failed to send bgp message");
        }
    }

    pub async fn send_withdraw(&self, prefix: Ipv4Addr, nexthop: Ipv4Addr, mut as_path: Vec<u32>) {
        let info = self.router_info.lock().await;
        as_path.insert(0, info.router_as);
        for (_, (_, sender, _, _)) in info.bgp_links.iter() {
            let message = BGPMessage::Withdraw(prefix, nexthop, as_path.clone());
            sender
                .send(Message::BGP(message))
                .await
                .expect("Failed to send bgp message");
        }
    }

    pub async fn announce_prefix(&self) {
        let info = self.router_info.lock().await;
        self.logger.borrow().log(Source::BGP, format!("Router {} announcing its prefix {}", info.name, info.ip)).await;
        let ip = info.ip;
        drop(info);
        self.send_update(ip, ip, vec![]).await;
    }
}
