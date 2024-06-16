use std::{fmt::{Display, Error}, net::Ipv4Addr, str::FromStr};

#[derive(Debug, PartialEq, Clone, Eq, Hash, Copy, Ord, PartialOrd)]
pub struct IPPrefix{
    pub ip: Ipv4Addr,
    pub prefix_len: u32,
}

impl Display for IPPrefix{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.ip, self.prefix_len)
    }
}

impl FromStr for IPPrefix{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s: Vec<&str> = s.split("/").collect();
        if s.len() != 2{
            return Err(Error);
        }

        let ip = s[0];
        let prefix_len = s[1];
        
        let ip = ip.parse();
        if ip.is_err(){
            return Err(Error);
        }
        let ip = ip.unwrap();

        let prefix_len = prefix_len.parse();
        if prefix_len.is_err(){
            return Err(Error);
        }
        let prefix_len = prefix_len.unwrap();
        if prefix_len > 32{
            return Err(Error);
        }

        Ok(IPPrefix{ip, prefix_len})
    }
}