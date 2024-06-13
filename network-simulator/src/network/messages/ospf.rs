use std::{collections::HashSet, net::Ipv4Addr};


#[derive(Debug, Clone)]
pub enum OSPFMessage{
    Hello,
    LSP(Ipv4Addr, u32, HashSet<(u32, Ipv4Addr)>),
    HelloReply(Ipv4Addr)
}