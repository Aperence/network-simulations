use std::{net::Ipv4Addr, rc::Rc};

type Child = Rc<IPTrieNode>;

#[derive(Debug)]
struct IPTrieNode {
    port: Option<u32>,
    left: Option<Child>,
    right: Option<Child>,
}

#[derive(Debug)]
pub struct IPTrie {
    root: Option<Child>,
}

impl IPTrie {
    pub fn new() -> IPTrie {
        IPTrie { root: Some(Rc::new(IPTrieNode{port: None, left: None, right: None})) }
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

    pub fn insert(&mut self, ip: Ipv4Addr, prefix_len: u32, port: u32) {
        let bits = self.bits(ip);

        self.root = Self::insert_node(self.root.clone(), bits, 0, prefix_len, port);
    }

    fn insert_node(
        node: Option<Child>,
        bits: Vec<bool>,
        idx: u32,
        prefix_len: u32,
        port: u32,
    ) -> Option<Child> {
        if idx == prefix_len{
            match node {
                Some(n) => Some(Rc::new(IPTrieNode {
                    port: Some(port),
                    left: n.left.clone(),
                    right: n.right.clone(),
                })),
                None => Some(Rc::new(IPTrieNode {
                    port: Some(port),
                    left: None,
                    right: None,
                })),
            }
        } else {
            match node {
                Some(n) => {
                    if bits[idx as usize] {
                        Some(Rc::new(IPTrieNode {
                            port: n.port,
                            left: n.left.clone(),
                            right: Self::insert_node(
                                n.right.clone(),
                                bits,
                                idx + 1,
                                prefix_len,
                                port,
                            ),
                        }))
                    } else {
                        Some(Rc::new(IPTrieNode {
                            port: n.port,
                            left: Self::insert_node(
                                n.left.clone(),
                                bits,
                                idx + 1,
                                prefix_len,
                                port,
                            ),
                            right: n.right.clone(),
                        }))
                    }
                }
                None => {
                    if bits[idx as usize] {
                        Some(Rc::new(IPTrieNode {
                            port: None,
                            left: None,
                            right: Self::insert_node(None, bits, idx + 1, prefix_len, port),
                        }))
                    } else {
                        Some(Rc::new(IPTrieNode {
                            port: None,
                            left: Self::insert_node(None, bits, idx + 1, prefix_len, port),
                            right: None,
                        }))
                    }
                }
            }
        }
    }

    pub fn get_port(&self, ip: Ipv4Addr) -> Option<u32> {
        let bits = self.bits(ip);
        let mut port = None;

        let mut curr = self.root.clone(); // clone a rc, cheap

        let mut idx = 0;
        while curr.is_some() {
            let n = curr.unwrap();

            if bits[idx] {
                curr = n.right.clone();
            } else {
                curr = n.left.clone();
            }

            idx += 1;
            if let Some(p) = n.port {
                port = Some(p);
            }
        }
        port
    }
}

#[cfg(test)]
mod tests {
    use super::IPTrie;

    #[test]
    fn test_trie() {

        let mut trie = IPTrie::new();

        trie.insert("10.0.0.0".parse().unwrap(), 24, 1); // 10.0.0.0/24
        trie.insert("10.0.0.128".parse().unwrap(), 25, 2); // 10.0.0.128/25
        trie.insert("255.248.0.15".parse().unwrap(), 31, 3); // 255.248.0.15/31
        trie.insert("128.0.0.0".parse().unwrap(), 1, 4); // 128.0.0.0/1

        assert_eq!(trie.get_port("10.0.0.64".parse().unwrap()), Some(1));
        assert_eq!(trie.get_port("10.0.0.164".parse().unwrap()), Some(2)); // longest match, return port 2 in priority
        assert_eq!(trie.get_port("255.248.0.15".parse().unwrap()), Some(3));
        assert_eq!(trie.get_port("192.168.0.1".parse().unwrap()), Some(4));
        assert_eq!(trie.get_port("11.0.0.64".parse().unwrap()), None);
    }

    #[test]
    fn test_default() {

        let mut trie = IPTrie::new();

        trie.insert("10.0.0.0".parse().unwrap(), 24, 1); // 10.0.0.0/24
        trie.insert("10.0.0.128".parse().unwrap(), 25, 2); // 10.0.0.128/25
        trie.insert("255.248.0.15".parse().unwrap(), 31, 3); // 255.248.0.15/31
        trie.insert("128.0.0.0".parse().unwrap(), 1, 4); // 128.0.0.0/1
        trie.insert("0.0.0.0".parse().unwrap(), 1, 5); // 0.0.0.0/0

        assert_eq!(trie.get_port("10.0.0.64".parse().unwrap()), Some(1));
        assert_eq!(trie.get_port("10.0.0.164".parse().unwrap()), Some(2)); // longest match, return port 2 in priority
        assert_eq!(trie.get_port("255.248.0.15".parse().unwrap()), Some(3));
        assert_eq!(trie.get_port("192.168.0.1".parse().unwrap()), Some(4));
        assert_eq!(trie.get_port("11.0.0.64".parse().unwrap()), Some(5));
        assert_eq!(trie.get_port("47.0.0.64".parse().unwrap()), Some(5));
    }
}
