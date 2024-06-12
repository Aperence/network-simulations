use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub enum BGPMessage{
    Update(Ipv4Addr, Ipv4Addr, Vec<u32>, u32, u32), // prefix, nexthop, as-path, med, router_id
    Withdraw(Ipv4Addr, Ipv4Addr, Vec<u32>, u32)     // prefix, nexthop, as-path, router_id
}