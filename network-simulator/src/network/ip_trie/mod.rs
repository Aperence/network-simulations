use std::sync::Arc;
use std::{fmt::Error, net::Ipv4Addr, str::FromStr};
use std::fmt::Display;

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

type Child<K> = Arc<IPTrieNode<K>>;

#[derive(Debug)]
struct IPTrieNode<K: Clone> {
    data: Option<K>,
    left: Option<Child<K>>,
    right: Option<Child<K>>,
}

#[derive(Debug)]
pub struct IPTrie<K: Clone> {
    root: Option<Child<K>>,
}

impl<K: Clone> IPTrie<K> {
    pub fn new() -> IPTrie<K> {
        IPTrie { root: Some(Arc::new(IPTrieNode{data: None, left: None, right: None})) }
    }

    fn bits(&self, ip: Ipv4Addr) -> Vec<bool> {
        let mut bits = vec![];
        for byte in ip.octets() {
            let mut mask = 1 << 7;
            while mask > 0 {
                bits.push((byte & mask) != 0);
                mask = mask >> 1;
            }
        }
        bits
    }

    pub fn insert(&mut self, prefix: IPPrefix, data: K) {
        let bits = self.bits(prefix.ip);

        self.root = Self::insert_node(self.root.clone(), bits, 0, prefix.prefix_len, data);
    }

    fn insert_node(
        node: Option<Child<K>>,
        bits: Vec<bool>,
        idx: u32,
        prefix_len: u32,
        data: K,
    ) -> Option<Child<K>> {
        if idx == prefix_len{
            match node {
                Some(n) => Some(Arc::new(IPTrieNode {
                    data: Some(data),
                    left: n.left.clone(),
                    right: n.right.clone(),
                })),
                None => Some(Arc::new(IPTrieNode {
                    data: Some(data),
                    left: None,
                    right: None,
                })),
            }
        } else {
            match node {
                Some(n) => {
                    if bits[idx as usize] {
                        Some(Arc::new(IPTrieNode {
                            data: n.data.clone(),
                            left: n.left.clone(),
                            right: Self::insert_node(
                                n.right.clone(),
                                bits,
                                idx + 1,
                                prefix_len,
                                data,
                            ),
                        }))
                    } else {
                        Some(Arc::new(IPTrieNode {
                            data: n.data.clone(),
                            left: Self::insert_node(
                                n.left.clone(),
                                bits,
                                idx + 1,
                                prefix_len,
                                data,
                            ),
                            right: n.right.clone(),
                        }))
                    }
                }
                None => {
                    if bits[idx as usize] {
                        Some(Arc::new(IPTrieNode {
                            data: None,
                            left: None,
                            right: Self::insert_node(None, bits, idx + 1, prefix_len, data),
                        }))
                    } else {
                        Some(Arc::new(IPTrieNode {
                            data: None,
                            left: Self::insert_node(None, bits, idx + 1, prefix_len, data),
                            right: None,
                        }))
                    }
                }
            }
        }
    }

    pub fn longest_match(&self, ip: Ipv4Addr) -> Option<K> {
        let bits = self.bits(ip);
        let mut data = None;

        let mut curr = self.root.clone(); // clone a rc, cheap

        let mut idx = 0;
        while curr.is_some(){
            let n = curr.unwrap();

            if let Some(p) = &n.data {
                data = Some(p.clone());
            }

            if idx == 32{
                break;
            }

            if bits[idx] {
                curr = n.right.clone();
            } else {
                curr = n.left.clone();
            }

            idx += 1;
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::IPTrie;

    #[test]
    fn test_trie() {

        let mut trie = IPTrie::new();

        trie.insert("10.0.0.0/24".parse().unwrap(), 1); 
        trie.insert("10.0.0.128/25".parse().unwrap(), 2); 
        trie.insert("255.248.0.15/31".parse().unwrap(), 3); 
        trie.insert("128.0.0.0/1".parse().unwrap(), 4); 
        trie.insert("255.248.0.16/32".parse().unwrap(), 5); 

        assert_eq!(trie.longest_match("10.0.0.64".parse().unwrap()), Some(1));
        assert_eq!(trie.longest_match("10.0.0.164".parse().unwrap()), Some(2)); // longest match, return port 2 in priority
        assert_eq!(trie.longest_match("255.248.0.15".parse().unwrap()), Some(3));
        assert_eq!(trie.longest_match("192.168.0.1".parse().unwrap()), Some(4));
        assert_eq!(trie.longest_match("255.248.0.16".parse().unwrap()), Some(5));
        assert_eq!(trie.longest_match("11.0.0.64".parse().unwrap()), None);
    }

    #[test]
    fn test_default() {

        let mut trie = IPTrie::new();

        trie.insert("10.0.0.0/24".parse().unwrap(), 1); 
        trie.insert("10.0.0.128/25".parse().unwrap(), 2); 
        trie.insert("255.248.0.15/31".parse().unwrap(), 3); 
        trie.insert("128.0.0.0/1".parse().unwrap(),  4); 
        trie.insert("0.0.0.0/0".parse().unwrap(),5);

        assert_eq!(trie.longest_match("10.0.0.64".parse().unwrap()), Some(1));
        assert_eq!(trie.longest_match("10.0.0.164".parse().unwrap()), Some(2)); // longest match, return port 2 in priority
        assert_eq!(trie.longest_match("255.248.0.15".parse().unwrap()), Some(3));
        assert_eq!(trie.longest_match("192.168.0.1".parse().unwrap()), Some(4));
        assert_eq!(trie.longest_match("11.0.0.64".parse().unwrap()), Some(5));
        assert_eq!(trie.longest_match("47.0.0.64".parse().unwrap()), Some(5));
    }
}
