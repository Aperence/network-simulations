use std::net::Ipv4Addr;

use crate::network::protocols::arp::MacAddress;

#[derive(Debug, Clone)]
pub enum ARPMessage{
    Request(Ipv4Addr),
    Reply(Ipv4Addr, MacAddress)
}