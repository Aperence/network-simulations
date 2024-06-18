use std::net::Ipv4Addr;

use crate::network::utils::MacAddress;

#[derive(Debug, Clone)]
pub enum ARPMessage{
    Request(Ipv4Addr),
    Reply(Ipv4Addr, MacAddress)
}