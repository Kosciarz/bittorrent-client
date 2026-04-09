use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug)]
pub struct Peer {
    socket: SocketAddr,
}

impl Peer {
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        Self {
            socket: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
                u16::from_ne_bytes([bytes[4], bytes[5]]),
            ),
        }
    }
}
