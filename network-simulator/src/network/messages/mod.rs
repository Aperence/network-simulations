pub mod bpdu;
pub mod ospf;
pub mod ip;
pub mod bgp;

use bpdu::BPDU;
use ospf::OSPFMessage;
use ip::IP;
use bgp::{BGPMessage, IBGPMessage};


#[derive(Debug, Clone)]
pub enum Message{
    BPDU(BPDU),
    OSPF(OSPFMessage),
    IP(IP),
    BGP(BGPMessage),
    IBGP(IBGPMessage)
}