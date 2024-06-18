use std::net::Ipv4Addr;

use crate::network::ip_prefix::IPPrefix;

#[derive(Debug, Clone)]
pub enum BGPMessage{
    Update(IPPrefix, Ipv4Addr, Vec<u32>, u32, u32), // prefix, nexthop, as-path, med, router_id
    Withdraw(IPPrefix, Ipv4Addr, Vec<u32>, u32)     // prefix, nexthop, as-path, router_id
}

#[derive(Debug, Clone)]
pub enum IBGPMessage{
    Update(IPPrefix, Ipv4Addr, Vec<u32>, u32, u32, u32), // prefix, nexthop, as-path, pref, med, router_id
    Withdraw(IPPrefix, Ipv4Addr, Vec<u32>, u32)     // prefix, nexthop, as-path, router_id
}