use std::{collections::HashSet, net::Ipv4Addr};

use crate::network::ip_prefix::IPPrefix;


#[derive(Debug, Clone)]
pub enum OSPFMessage{
    Hello,
    LSP(Ipv4Addr, u32, HashSet<(u32, IPPrefix)>),
    HelloReply(IPPrefix)
}