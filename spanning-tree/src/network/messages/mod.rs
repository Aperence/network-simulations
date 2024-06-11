pub mod bpdu;
pub mod ospf;

use bpdu::BPDU;
use ospf::OSPFMessage;

#[derive(Debug)]
pub enum Message{
    BPDU(BPDU),
    OSPF(OSPFMessage)
}