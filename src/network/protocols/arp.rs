use std::{collections::HashMap, net::Ipv4Addr};

use crate::network::{logger::{Logger, Source}, messages::{arp::ARPMessage, Message}, router::RouterInfo, utils::{MacAddress, SharedState}};

#[derive(Debug)]
pub struct ArpState{
    pub mapping: HashMap<Ipv4Addr, MacAddress>,
    pub router_info: SharedState<RouterInfo>,
    pub logger: Logger
}

impl ArpState{
    pub fn new(router_info: SharedState<RouterInfo>, logger: Logger) -> ArpState{
        ArpState{mapping: HashMap::new(), router_info, logger}
    }

    pub async fn resolve(&self, ip: Ipv4Addr, port: u32){
        self.logger.log(Source::ARP, format!("Router {} sending resolving request for {}", self.router_info.lock().await.name, ip)).await;
        let info = self.router_info.lock().await;
        if let Some((_, sender)) = info.neighbors_links.get(&port){
            sender.send(Message::ARP(ARPMessage::Request(ip))).await.expect("Failed to send arp message");
        }
    }

    pub async fn process_request(&mut self, ip: Ipv4Addr, port: u32){
        self.logger.log(Source::ARP, format!("Router {} received request for mapping of ip {}", self.router_info.lock().await.name, ip)).await;
        let info = self.router_info.lock().await;
        if info.ip != ip{
            return;
        }
        if let Some((_, sender)) = info.neighbors_links.get(&port){
            sender.send(Message::ARP(ARPMessage::Reply(ip, info.mac_address.clone()))).await.expect("Failed to send arp message");
        }
    }

    pub async fn process_reply(&mut self, ip: Ipv4Addr, mac_address: MacAddress){
        self.mapping.insert(ip, mac_address);
        self.logger.log(Source::ARP, format!("Router {} has mappings : {:?}", self.router_info.lock().await.name, self.mapping)).await;
    }

    pub async fn process_arp_message(&mut self, arp_message: ARPMessage, port: u32){
        match arp_message {
            ARPMessage::Request(ip) => self.process_request(ip, port).await,
            ARPMessage::Reply(ip, mac) => self.process_reply(ip, mac).await,
        }
    }
}