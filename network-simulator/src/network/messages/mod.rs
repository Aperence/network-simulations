pub mod bpdu;
pub mod ospf;
pub mod ip;
pub mod bgp;
pub mod arp;

use arp::ARPMessage;
use bpdu::BPDU;
use ospf::OSPFMessage;
use ip::IP;
use bgp::BGPMessage;

use super::protocols::arp::MacAddress;


#[derive(Debug, Clone)]
pub enum Message{
    BPDU(BPDU),
    OSPF(OSPFMessage),
    EthernetFrame(MacAddress, IP),
    BGP(BGPMessage),
    ARP(ARPMessage)
}