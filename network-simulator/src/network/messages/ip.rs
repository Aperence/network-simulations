use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub enum Content{
    Ping,
    Pong,
    Data(String)
}

#[derive(Debug, Clone)]
pub struct IP{
    pub src: Ipv4Addr, 
    pub dest: Ipv4Addr,
    pub content: Content
}