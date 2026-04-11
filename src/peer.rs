use std::{
    io::{self, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};

#[derive(Debug)]
pub struct Peer {
    socket: SocketAddr,
    chocked: bool,
    interested: bool,
}

impl Peer {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            socket: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
                u16::from_be_bytes([bytes[4], bytes[5]]),
            ),
            chocked: true,
            interested: false,
        }
    }

    pub fn chocked(&self) -> bool {
        self.chocked
    }

    pub fn interested(&self) -> bool {
        self.interested
    }

    pub fn connect(&mut self, info_hash: &[u8], client_id: &[u8]) -> io::Result<()> {
        let mut connection = PeerConnection::new(&self.socket)?;
        println!("Connected to peer: {}", self.socket);

        connection.send_handshake(info_hash, client_id)?;

        Ok(())
    }
}

#[derive(Debug)]
struct PeerConnection {
    stream: TcpStream,
    handshake_sent: bool,
}

impl PeerConnection {
    fn new(socket: &SocketAddr) -> io::Result<Self> {
        Ok(Self {
            stream: TcpStream::connect_timeout(socket, Duration::from_secs(5))?,
            handshake_sent: false,
        })
    }

    fn send_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        Ok(self.stream.write_all(bytes)?)
    }

    fn encode_handshake(info_hash: &[u8], client_id: &[u8]) -> Vec<u8> {
        let mut handshake = Vec::with_capacity(68);

        handshake.push(19);
        handshake.extend_from_slice(b"BitTorrent protocol");
        handshake.extend_from_slice(&[0u8; 8]);
        handshake.extend_from_slice(info_hash);
        handshake.extend_from_slice(client_id);

        handshake
    }

    fn send_handshake(&mut self, info_hash: &[u8], client_id: &[u8]) -> io::Result<()> {
        if self.handshake_sent {
            return Ok(());
        }

        self.send_bytes(&Self::encode_handshake(info_hash, client_id))?;
        self.handshake_sent = true;

        println!("-> handshake");

        Ok(())
    }
}
