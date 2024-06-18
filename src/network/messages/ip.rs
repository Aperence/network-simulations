use std::net::Ipv4Addr;

use super::bgp::IBGPMessage;

#[derive(Debug, Clone)]
pub enum Content{
    Ping,
    Pong,
    Data(String),
    IBGP(IBGPMessage)
}

#[derive(Debug, Clone)]
pub struct IP{
    pub src: Ipv4Addr, 
    pub dest: Ipv4Addr,
    pub content: Content
}