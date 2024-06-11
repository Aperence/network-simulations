pub mod bpdu;
pub mod ospf;

use std::net::Ipv4Addr;

use bpdu::BPDU;
use ospf::OSPFMessage;

#[derive(Debug)]
pub enum DebugMessage{
    Ping(Ipv4Addr, Ipv4Addr),
    Pong(Ipv4Addr, Ipv4Addr)
}

#[derive(Debug)]
pub enum Message{
    BPDU(BPDU),
    OSPF(OSPFMessage),
    Debug(DebugMessage)
}