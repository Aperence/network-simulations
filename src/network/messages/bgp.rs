use std::{fmt::Display, net::Ipv4Addr};

use crate::network::ip_prefix::IPPrefix;

#[derive(Debug, Clone)]
pub enum BGPMessage{
    Update(IPPrefix, Ipv4Addr, Vec<u32>, u32, u32), // prefix, nexthop, as-path, med, router_id
    Withdraw(IPPrefix, Ipv4Addr, Vec<u32>, u32)     // prefix, nexthop, as-path, router_id
}

impl Display for BGPMessage{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            BGPMessage::Update(prefix, nexthop, as_path, med, router_id) => 
                write!(f, "UPDATE(prefix={}, nexthop={}, as_path={}, med={}, router_id={})", 
                    prefix, nexthop, as_path.iter().map(|a| format!("AS{}", a)).collect::<Vec<String>>().join(":"), med, router_id),
            BGPMessage::Withdraw(prefix, nexthop, as_path, router_id) =>                 
                write!(f, "WITHDRAW(prefix={}, nexthop={}, as_path={}, router_id={})", 
                    prefix, nexthop, as_path.iter().map(|a| format!("AS{}", a)).collect::<Vec<String>>().join(":"), router_id)
        }
    }
}

#[derive(Debug, Clone)]
pub enum IBGPMessage{
    Update(IPPrefix, Ipv4Addr, Vec<u32>, u32, u32, u32), // prefix, nexthop, as-path, pref, med, router_id
    Withdraw(IPPrefix, Ipv4Addr, Vec<u32>, u32)     // prefix, nexthop, as-path, router_id
}

impl Display for IBGPMessage{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            IBGPMessage::Update(prefix, nexthop, as_path, pref, med, router_id) => 
                write!(f, "UPDATE(prefix={}, nexthop={}, as_path={}, pref={}, med={}, router_id={})", 
                    prefix, nexthop, as_path.iter().map(|a| format!("AS{}", a)).collect::<Vec<String>>().join(":"), pref, med, router_id),
            IBGPMessage::Withdraw(prefix, nexthop, as_path, router_id) =>                 
                write!(f, "WITHDRAW(prefix={}, nexthop={}, as_path={}, router_id={})", 
                    prefix, nexthop, as_path.iter().map(|a| format!("AS{}", a)).collect::<Vec<String>>().join(":"), router_id)
        }
    }
}