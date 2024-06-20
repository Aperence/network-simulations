use std::{borrow::Borrow, collections::{hash_map::Entry, HashMap, HashSet}, fmt::Display, net::Ipv4Addr};

use crate::network::{
    ip_prefix::IPPrefix, ip_trie::IPTrie, logger::{Logger, Source}, messages::{bgp::{BGPMessage, IBGPMessage}, ip::{Content, IP}, Message}, router::RouterInfo, utils::SharedState
};

use super::ospf::OSPFState;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum RouteSource{
    IBGP,
    EBGP
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct BGPRoute{
    pub prefix: IPPrefix,
    pub nexthop: Ipv4Addr,
    pub as_path: Vec<u32>,
    pub pref: u32,
    pub med: u32,
    pub router_id: u32,
    pub source: RouteSource
}

impl Display for BGPRoute{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.as_path.iter().map(|v| format!("AS{}", v)).collect::<Vec<String>>().join(":");
        write!(f, "nexthop={}, AS path={}, pref={}, med={}", self.nexthop, path, self.pref, self.med)
    }
}

#[derive(Debug)]
pub struct BGPState {
    pub router_info: SharedState<RouterInfo>,
    pub igp_info: SharedState<OSPFState>,
    pub logger: Logger,
    pub routes: HashMap<IPPrefix, HashSet<BGPRoute>>,
    pub prefixes: IPTrie<IPPrefix>
}

impl BGPState {
    pub fn new(router_info: SharedState<RouterInfo>, igp_info: SharedState<OSPFState>, logger: Logger) -> BGPState {
        BGPState {
            router_info,
            igp_info,
            logger,
            routes: HashMap::new(),
            prefixes: IPTrie::new()
        }
    }

    pub async fn process_bgp_message(&mut self, port:u32, message: BGPMessage) {
        match message {
            BGPMessage::Update(prefix, nexthop, as_path, med, router_id) => {
                self.process_update(port, prefix, nexthop, as_path, med, router_id).await
            }
            BGPMessage::Withdraw(prefix, nexthop, as_path, router_id) => {
                self.process_withdraw(port, prefix, nexthop, as_path, router_id).await
            }
        }
    }

    pub async fn process_ibgp_message(&mut self, port:u32, message: IBGPMessage) {
        match message {
            IBGPMessage::Update(prefix, nexthop, as_path, pref, med, router_id) => {
                self.process_update_ibgp(port, prefix, nexthop, as_path, pref, med, router_id).await
            }
            IBGPMessage::Withdraw(prefix, nexthop, as_path, router_id) => {
                self.process_withdraw_ibgp(port, prefix, nexthop, as_path, router_id).await
            }
        }
    }

    pub async fn install_route(&self, route: BGPRoute){
        let mut igp_state = self.igp_info.lock().await;
        let port = igp_state.get_port(route.nexthop).await.unwrap().clone();
        igp_state.routing_table.insert(route.prefix, (port, 0));
    }

    pub async fn process_update(
        &mut self,
        port: u32,
        prefix: IPPrefix,
        nexthop: Ipv4Addr,
        as_path: Vec<u32>,
        med: u32,
        router_id: u32
    ) {
        
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        let ip = info.ip;
        let pref = info.bgp_links.get(&port).unwrap().0;
        let current_as = info.router_as;
        drop(info);
        if as_path.contains(&current_as){
            return;
        }
        self.prefixes.insert(prefix, prefix);
        self.logger.borrow().log(Source::BGP, format!("Router {} received bgp update on port {} for prefix {} with nexthop = {}, AS path = {:?}, med = {}", name, port, prefix, nexthop, as_path, med)).await;
        let route = BGPRoute{prefix, nexthop, as_path, pref, med, source: RouteSource::EBGP, router_id};

        let previous_best = self.decision_process(prefix).await;

        let routes = match self.routes.entry(prefix) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        routes.insert(route);

        let best = self.decision_process(prefix).await;

        if previous_best != best{
            if let Some(previous_best_route) = previous_best{
                self.send_withdraw(previous_best_route.prefix, ip, previous_best_route.as_path.clone()).await;
                if previous_best_route.source != RouteSource::IBGP{
                    self.send_ibgp_withdraw(previous_best_route.prefix, previous_best_route.as_path).await;
                }
            }
            let best = best.unwrap();
            self.logger.borrow().log(Source::BGP, format!("Router {} has new best route ({}) to reach prefix {}", name, best, best.prefix)).await;
            self.install_route(best.clone()).await;
            self.send_update(best.prefix, ip, best.as_path.clone(), best.pref).await;
            self.send_ibgp_update(best.prefix, best.as_path, best.pref, best.med).await;
        }
    }

    pub async fn process_withdraw(&mut self, port: u32, prefix: IPPrefix, nexthop: Ipv4Addr, as_path: Vec<u32>, router_id: u32) {
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        let current_as = info.router_as;
        let ip = info.ip;
        drop(info);
        if as_path.contains(&current_as){
            return;
        }
        self.logger.borrow().log(Source::BGP, format!("Router {} received bgp withdraw on port {} for prefix {} with nexthop = {}, AS path = {:?}", name, port, prefix, nexthop, as_path)).await;
    
        let previous_best = self.decision_process(prefix).await;

        let routes = self.routes.get(&prefix);

        if let None = routes{
            return;
        }

        let routes = routes.unwrap();

        let mut new_routes = HashSet::new();
        let mut best_removed = false;
        for route in routes{
            if route.nexthop == nexthop && route.router_id == router_id && route.as_path == as_path{
                if let Some(r) = &previous_best{
                    best_removed = best_removed || route.nexthop == r.nexthop && route.router_id == r.router_id && route.as_path == r.as_path ; 
                }
            }else{
                new_routes.insert(route.clone());
            }
        }
        
        self.routes.insert(prefix, new_routes);

        if best_removed{
            let previous_best = previous_best.unwrap();
            self.send_withdraw(prefix, ip, previous_best.as_path.clone()).await;
            if previous_best.source == RouteSource::EBGP{
                self.send_ibgp_withdraw(prefix, previous_best.as_path).await;
            }

            let new_best = self.decision_process(prefix).await;
            if let Some(new_best_route) = new_best{
                self.logger.borrow().log(Source::BGP, format!("Router {} has new best route ({}) to reach prefix {}", name, new_best_route, new_best_route.prefix)).await;
                self.install_route(new_best_route.clone()).await;
                self.send_update(prefix, ip, new_best_route.as_path.clone(), new_best_route.pref).await;
                if new_best_route.source != RouteSource::IBGP{
                    self.send_ibgp_update(new_best_route.prefix, new_best_route.as_path, new_best_route.pref, new_best_route.med).await;
                }
            }
        }
        
    }

    pub async fn process_update_ibgp(
        &mut self,
        port: u32,
        prefix: IPPrefix,
        nexthop: Ipv4Addr,
        as_path: Vec<u32>,
        pref: u32,
        med: u32,
        router_id: u32
    ){
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        let ip = info.ip;
        drop(info);
        self.prefixes.insert(prefix, prefix);
        self.logger.borrow().log(Source::BGP, format!("Router {} received ibgp update on port {} for prefix {} with nexthop = {}, AS path = {:?}, med = {}", name, port, prefix, nexthop, as_path, med)).await;
        let route = BGPRoute{prefix, nexthop, as_path, pref, med, source: RouteSource::IBGP, router_id};

        let previous_best = self.decision_process(prefix).await;

        let routes = match self.routes.entry(prefix) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(HashSet::new()),
        };

        routes.insert(route);

        let best = self.decision_process(prefix).await;

        if previous_best != best{
            if let Some(previous_best_route) = previous_best{
                self.send_withdraw(previous_best_route.prefix, ip, previous_best_route.as_path.clone()).await;
                if previous_best_route.source != RouteSource::IBGP{
                    self.send_ibgp_withdraw(previous_best_route.prefix, previous_best_route.as_path).await;
                }
            }
            let best = best.unwrap();
            self.logger.borrow().log(Source::BGP, format!("Router {} has new best route ({}) to reach prefix {}", name, best, best.prefix)).await;
            self.install_route(best.clone()).await;
            self.send_update(best.prefix, ip, best.as_path.clone(), best.pref).await;
            // suppose fullmesh, no need to readvertise new best to other ibgp peers
        }
    }

    pub async fn process_withdraw_ibgp(&mut self, port: u32, prefix: IPPrefix, nexthop: Ipv4Addr, as_path: Vec<u32>, router_id: u32) {
        let info = self.router_info.lock().await;
        let name = info.name.clone();
        let ip = info.ip;
        drop(info);
        self.logger.borrow().log(Source::BGP, format!("Router {} received ibgp withdraw on port {} for prefix {} with nexthop = {}, AS path = {:?}", name, port, prefix, nexthop, as_path)).await;
    
        let previous_best = self.decision_process(prefix).await;

        let routes = self.routes.get(&prefix);

        if let None = routes{
            return;
        }

        let routes = routes.unwrap();

        let mut new_routes = HashSet::new();
        let mut best_removed = false;
        for route in routes{
            if route.nexthop == nexthop && route.router_id == router_id && route.as_path == as_path{
                if let Some(r) = &previous_best{
                    best_removed = best_removed || route.nexthop == r.nexthop && route.router_id == r.router_id && route.as_path == r.as_path ; 
                }
            }else{
                new_routes.insert(route.clone());
            }
        }
        
        self.routes.insert(prefix, new_routes);

        if best_removed{
            let previous_best = previous_best.unwrap();
            self.send_withdraw(prefix, ip, previous_best.as_path.clone()).await;
            if previous_best.source == RouteSource::EBGP{
                self.send_ibgp_withdraw(prefix, previous_best.as_path).await;
            }

            let new_best = self.decision_process(prefix).await;
            if let Some(new_best_route) = new_best{
                self.logger.borrow().log(Source::BGP, format!("Router {} has new best route ({}) to reach prefix {}", name, new_best_route, new_best_route.prefix)).await;
                self.install_route(new_best_route.clone()).await;
                self.send_update(prefix, ip, new_best_route.as_path.clone(), new_best_route.pref).await;
                if new_best_route.source != RouteSource::IBGP{
                    self.send_ibgp_update(new_best_route.prefix, new_best_route.as_path, new_best_route.pref, new_best_route.med).await;
                }
            }
        }
    }

    pub async fn distance_nexthop(&self, nexthop: Ipv4Addr) -> u32{
        let igp_info = &self.igp_info.lock().await;
        let prefix = igp_info.prefixes.longest_match(nexthop);
        if prefix.is_none(){
            return u32::max_value();
        }
        let prefix = prefix.unwrap();
        match igp_info.routing_table.get(&prefix){
            Some((_, distance)) => *distance,
            None => u32::max_value(),
        }
    }

    pub async fn decision_process(&self, prefix: IPPrefix) -> Option<BGPRoute>{
        let routes = self.routes.get(&prefix);

        if routes.is_none(){
            return None;
        }

        let routes = routes.unwrap();

        if routes.is_empty(){
            return None;
        }

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
            let map_entry = match map.entry(route.as_path[0]) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(vec![]),
            };

            if map_entry.len() == 0{
                map_entry.push(route);
            }else if map_entry[0].med > route.med{
                map_entry.clear();
                map_entry.push(route);
            }else if map_entry[0].med == route.med{
                map_entry.push(route);
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
            }else if route.router_id < best_route.router_id{
                    best_route = route;
            }
        }

        Some(best_route.clone())
    }

    pub async fn send_update(&self, prefix: IPPrefix, nexthop: Ipv4Addr, mut as_path: Vec<u32>, pref_from: u32) {
        let info = self.router_info.lock().await;
        as_path.insert(0, info.router_as);
        for (port, (pref, med)) in info.bgp_links.iter() {
            let (_, sender) = info.neighbors_links.get(port).unwrap();
            if pref_from != 150 && *pref != 150{
                // send routes from peer/providers only to customers
                continue;
            }
            let message = BGPMessage::Update(prefix.clone(), nexthop, as_path.clone(), *med, info.id);
            self.logger.borrow().log(Source::BGP, format!("Router {} has sent {} on port {}", info.name, message, port)).await;
            sender
                .send(Message::BGP(message))
                .await
                .expect("Failed to send bgp message");
        }
    }

    pub async fn send_ibgp_update(&self, prefix: IPPrefix, as_path: Vec<u32>, pref_from: u32, med: u32) {
        let igp_state = self.igp_info.lock().await;
        let info =  self.router_info.lock().await;
        let peers = info.ibgp_peers.clone();
        let self_ip = info.ip;
        let self_id = info.id;
        let name = info.name.clone();
        drop(info);
        for peer_addr in peers {
            let ibgp_message = IBGPMessage::Update(prefix.clone(), self_ip, as_path.clone(), pref_from, med, self_id);
            self.logger.borrow().log(Source::BGP, format!("Router {} has sent iBGP message {} to peer {}", name, ibgp_message, peer_addr)).await;
            let message = IP{
                src: self_ip, 
                dest: peer_addr.clone(), 
                content: Content::IBGP(ibgp_message)
            };
            igp_state.send_message(peer_addr.clone(), message).await;
        }
    }

    pub async fn send_withdraw(&self, prefix: IPPrefix, nexthop: Ipv4Addr, mut as_path: Vec<u32>) {
        let info = self.router_info.lock().await;
        as_path.insert(0, info.router_as);
        for (port, _) in info.bgp_links.iter() {
            let (_, sender) = info.neighbors_links.get(port).unwrap();
            let message = BGPMessage::Withdraw(prefix.clone(), nexthop, as_path.clone(), info.id);
            self.logger.borrow().log(Source::BGP, format!("Router {} has sent {} on port {}", info.name, message, port)).await;
            sender
                .send(Message::BGP(message))
                .await
                .expect("Failed to send bgp message");
        }
    }

    pub async fn send_ibgp_withdraw(&self, prefix: IPPrefix, as_path: Vec<u32>) {
        let igp_state = self.igp_info.lock().await;
        let info =  self.router_info.lock().await;
        let peers = info.ibgp_peers.clone();
        let self_ip = info.ip;
        let self_id = info.id;
        let name = info.name.clone();
        drop(info);
        for peer_addr in peers {
            let ibgp_message = IBGPMessage::Withdraw(prefix.clone(), self_ip, as_path.clone(), self_id);
            self.logger.borrow().log(Source::BGP, format!("Router {} has sent iBGP message {} to peer {}", name, ibgp_message, peer_addr)).await;
            let message = IP{
                src: self_ip, 
                dest: peer_addr.clone(), 
                content: Content::IBGP(ibgp_message)
            };
            igp_state.send_message(peer_addr.clone(), message).await;
        }
    }


    pub async fn announce_prefix(&self) {
        let info = self.router_info.lock().await;
        self.logger.borrow().log(Source::BGP, format!("Router {} announcing its prefix {}", info.name, info.ip)).await;
        let ip = info.ip;
        drop(info);
        let octets = ip.octets();
        let prefix = IPPrefix{ip: Ipv4Addr::new(octets[0], octets[1], octets[2], 0), prefix_len: 24};
        self.send_update(prefix, ip, vec![], 150).await;
    }

    pub async fn get_nexthop(&self, dest: Ipv4Addr) -> Option<Ipv4Addr>{
        let prefix = self.prefixes.longest_match(dest)?;
        let best_route = self.decision_process(prefix).await?;
        Some(best_route.nexthop)
    }
}
