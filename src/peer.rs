use std::{net::SocketAddr, time::Duration};

use anyhow::{Result, anyhow};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const HANDSHAKE_SIZE: usize = 68;

#[derive(Debug, Clone)]
pub struct BitField {
    pub bytes: Vec<u8>,
    pub num_pieces: usize,
}

impl BitField {
    pub fn new(bytes: Vec<u8>, num_pieces: usize) -> Self {
        Self { bytes, num_pieces }
    }

    pub fn has_piece(&self, index: usize) -> bool {
        if index >= self.num_pieces {
            return false;
        }

        let byte = index / 8;
        let bit = 7 - (index % 8);
        self.bytes[byte] & (1 << bit) != 0
    }
}

#[derive(Debug, Clone)]
pub struct Peer {
    addr: SocketAddr,
    bitfield: Option<BitField>,
    peer_id: [u8; 20],
    chocked: bool,
    interested: bool,
}

impl Peer {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            bitfield: None,
            chocked: true,
            interested: false,
            peer_id: [0u8; 20],
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn chocked(&self) -> bool {
        self.chocked
    }

    pub fn interested(&self) -> bool {
        self.interested
    }

    pub fn set_bitfield(&mut self, bitfield: BitField) {
        self.bitfield = Some(bitfield);
    }

    pub fn has_bitfield(&self) -> bool {
        self.bitfield.is_some()
    }
}

#[derive(Debug)]
pub struct PeerConnection {
    peer: Peer,
    stream: TcpStream,
    num_pieces: usize,
}

impl PeerConnection {
    pub async fn connect(
        mut peer: Peer,
        info_hash: &[u8; 20],
        peer_id: &[u8; 20],
        num_pieces: usize,
    ) -> Result<Self, (Peer, anyhow::Error)> {
        let mut stream =
            match timeout(Duration::from_secs(5), TcpStream::connect(&peer.addr())).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => return Err((peer, e.into())),
                Err(_) => return Err((peer, anyhow!("connection timed out"))),
            };

        let handshake = &Self::build_handshake(info_hash, peer_id);
        if let Err(e) = stream.write_all(handshake).await {
            return Err((peer, e.into()));
        }

        let mut buf = [0u8; HANDSHAKE_SIZE];
        if let Err(e) = stream.read_exact(&mut buf).await {
            return Err((peer, e.into()));
        }

        if buf[0] != 19 {
            return Err((peer, anyhow!("Invalid pstrlen")));
        }

        if &buf[1..20] != b"BitTorrent protocol" {
            return Err((peer, anyhow!("Invalid pstr")));
        }

        if &buf[28..48] != info_hash {
            return Err((peer, anyhow!("Info hash does not match")));
        }

        peer.peer_id = buf[48..68].try_into().unwrap();

        println!("Connected to peer: {}", peer.addr());

        Ok(PeerConnection {
            peer,
            stream,
            num_pieces,
        })
    }

    fn build_handshake(info_hash: &[u8], client_id: &[u8]) -> Vec<u8> {
        let mut handshake = Vec::with_capacity(HANDSHAKE_SIZE);

        handshake.push(19);
        handshake.extend_from_slice(b"BitTorrent protocol");
        handshake.extend_from_slice(&[0u8; 8]);
        handshake.extend_from_slice(info_hash);
        handshake.extend_from_slice(client_id);

        handshake
    }

    pub async fn send_message(&mut self, message: Message) -> Result<()> {
        Ok(self.stream.write_all(&message.encode()).await?)
    }

    pub async fn read_message(&mut self) -> Result<Message> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);

        if len == 0 {
            return Ok(Message::KeepAlive);
        }

        let mut buf = vec![0u8; len.try_into().unwrap()];
        self.stream.read_exact(&mut buf).await?;
        Message::decode(&buf)
    }

    pub async fn read_first_message(&mut self) -> Result<()> {
        loop {
            let message = self.read_message().await?;

            match &message {
                Message::KeepAlive => continue,
                Message::Unchoke => self.peer.chocked = false,
                Message::Bitfield(b) => {
                    self.peer.bitfield = Some(BitField::new(b.to_vec(), self.num_pieces));
                }
                _ => break,
            }

            println!("{:?}", message);

            if self.peer.has_bitfield() && !self.peer.chocked() {
                break;
            }
        }

        Ok(())
    }

    pub async fn send_interested(&mut self) -> Result<()> {
        self.send_message(Message::Interested).await
    }
}

#[derive(Debug)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
}

impl Message {
    fn encode(&self) -> Vec<u8> {
        match self {
            Message::KeepAlive => Self::encode_keep_alive(),
            Message::Choke => Self::encode_state(0),
            Message::Unchoke => Self::encode_state(1),
            Message::Interested => Self::encode_state(2),
            Message::NotInterested => Self::encode_state(3),
            Message::Have(piece_index) => Self::encode_have(*piece_index),
            Message::Bitfield(bitfield) => Self::encode_bitfield(bitfield),
            Message::Request {
                index,
                begin,
                length,
            } => Self::encode_request(*index, *begin, *length),
            Message::Piece {
                index,
                begin,
                block,
            } => Self::encode_piece(*index, *begin, block),
            Message::Cancel {
                index,
                begin,
                length,
            } => Self::encode_cancel(*index, *begin, *length),
        }
    }

    fn encode_keep_alive() -> Vec<u8> {
        0_u32.to_be_bytes().to_vec()
    }

    fn encode_state(id: u8) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);

        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.push(id);

        buf
    }

    fn encode_have(piece_index: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 5);

        buf.extend_from_slice(&5u32.to_be_bytes());
        buf.push(4);
        buf.extend_from_slice(&piece_index.to_be_bytes());

        buf
    }

    fn encode_bitfield(bitfield: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 1 + bitfield.len());
        let length = 1 + bitfield.len();

        buf.extend_from_slice(&(length as u32).to_be_bytes());
        buf.push(5);
        buf.extend_from_slice(bitfield);

        buf
    }

    fn encode_request(index: u32, begin: u32, length: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 13);

        buf.extend_from_slice(&13u32.to_be_bytes());
        buf.push(6);
        buf.extend_from_slice(&index.to_be_bytes());
        buf.extend_from_slice(&begin.to_be_bytes());
        buf.extend_from_slice(&length.to_be_bytes());

        buf
    }

    fn encode_piece(index: u32, begin: u32, block: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 9 + block.len());
        let length = 9 + block.len();

        buf.extend_from_slice(&(length as u32).to_be_bytes());
        buf.push(7);
        buf.extend_from_slice(&index.to_be_bytes());
        buf.extend_from_slice(&begin.to_be_bytes());
        buf.extend_from_slice(block);

        buf
    }

    fn encode_cancel(index: u32, begin: u32, length: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 13);

        buf.extend_from_slice(&13u32.to_be_bytes());
        buf.push(8);
        buf.extend_from_slice(&index.to_be_bytes());
        buf.extend_from_slice(&begin.to_be_bytes());
        buf.extend_from_slice(&length.to_be_bytes());

        buf
    }

    fn decode(buf: &[u8]) -> Result<Message> {
        let id = buf.first().ok_or_else(|| anyhow!("Id missing"))?;
        let buf = &buf[1..];

        Ok(match id {
            0 => Message::Choke,
            1 => Message::Unchoke,
            2 => Message::Interested,
            3 => Message::NotInterested,
            4 => Self::decode_have(buf),
            5 => Self::decode_bitfield(buf),
            6 => Self::decode_request(buf),
            7 => Self::decode_piece(buf),
            8 => Self::decode_cancel(buf),
            _ => return Err(anyhow!("Invalid id")),
        })
    }

    fn decode_have(buf: &[u8]) -> Message {
        let piece_index = u32::from_be_bytes(buf.try_into().unwrap());

        Message::Have(piece_index)
    }

    fn decode_bitfield(buf: &[u8]) -> Message {
        Message::Bitfield(buf.to_vec())
    }

    fn decode_request(buf: &[u8]) -> Message {
        let (chunks, _) = buf.as_chunks::<4>();

        let index = u32::from_be_bytes(chunks[0]);
        let begin = u32::from_be_bytes(chunks[1]);
        let length = u32::from_be_bytes(chunks[2]);

        Message::Request {
            index,
            begin,
            length,
        }
    }

    fn decode_piece(buf: &[u8]) -> Message {
        let (head, block) = buf.split_at(8);
        let (chunks, _) = head.as_chunks::<4>();

        let index = u32::from_be_bytes(chunks[0]);
        let begin = u32::from_be_bytes(chunks[1]);
        let block = block.to_vec();

        Message::Piece {
            index,
            begin,
            block,
        }
    }

    fn decode_cancel(buf: &[u8]) -> Message {
        let (chunks, _) = buf.as_chunks::<4>();

        let index = u32::from_be_bytes(chunks[0]);
        let begin = u32::from_be_bytes(chunks[1]);
        let length = u32::from_be_bytes(chunks[2]);

        Message::Cancel {
            index,
            begin,
            length,
        }
    }
}
