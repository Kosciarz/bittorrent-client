use std::{collections::BTreeMap, fs, io, net::SocketAddr, path::Path, time::Duration};

use anyhow::{Context, Result, anyhow};
use sha1::{Digest, Sha1};
use tokio::time::timeout;
use url::Url;

use crate::{
    bencode::{
        self, Object, ObjectType, decode_object,
        object::{extract_byte_array, extract_dict, extract_list, extract_num, extract_str},
    },
    client::Client,
    peer::{BitField, Peer, PeerConnection},
    tracker::{AnnounceStats, Tracker},
};

pub const BLOCK_SIZE: u32 = 16_384;

pub struct Torrent {
    tracker: Tracker,
    announce_list: Vec<Vec<Tracker>>,
    comment: String,
    created_by: String,
    creation_date: u64,

    name: String,
    length: u64,
    piece_length: u64,
    pieces: Vec<[u8; 20]>,
    info_hash: [u8; 20],

    downloaded: u64,
    left: u64,
    uploaded: u64,

    peers: Vec<Peer>,
    have: Vec<bool>,
}

impl Torrent {
    pub fn announce(&self) -> &Tracker {
        &self.tracker
    }

    pub fn announce_list(&self) -> &Vec<Vec<Tracker>> {
        &self.announce_list
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn created_by(&self) -> &str {
        &self.created_by
    }

    pub fn creation_date(&self) -> u64 {
        self.creation_date
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn piece_length(&self) -> u64 {
        self.piece_length
    }

    pub fn pieces(&self) -> &[[u8; 20]] {
        &self.pieces
    }

    pub fn info_hash(&self) -> &[u8; 20] {
        &self.info_hash
    }

    pub fn downloaded(&self) -> u64 {
        self.downloaded
    }

    pub fn left(&self) -> u64 {
        self.left
    }

    pub fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub fn load_from_file(path: &Path) -> Result<Torrent> {
        let bytes = fs::read(path)?;
        let obj = decode_object(&bytes);
        Torrent::try_from(obj)
    }

    pub fn save_to_file(torrent: &Torrent, path: &Path) -> io::Result<()> {
        let obj = Object::from(torrent);
        let bytes = bencode::encode_object(&obj);
        fs::write(
            format!(
                "{}/{}.torrent",
                path.to_string_lossy().to_string(),
                torrent.name()
            ),
            bytes,
        )?;
        Ok(())
    }

    pub async fn update_trackers(&mut self, client: &Client) -> Result<()> {
        let addrs = self
            .tracker
            .announce(
                &self.info_hash,
                &client.peer_id,
                client.port,
                &AnnounceStats {
                    uploaded: self.uploaded,
                    downloaded: self.downloaded,
                    left: self.left,
                },
            )
            .await?;

        self.add_peers(addrs);

        Ok(())
    }

    pub fn add_peers(&mut self, addrs: Vec<SocketAddr>) {
        for addr in addrs {
            if !self.peers.iter().any(|p| p.addr() == addr) {
                self.peers.push(Peer::new(addr));
            }
        }
    }

    pub async fn connect_peers(&mut self, client: &Client) -> Result<()> {
        let peers: Vec<Peer> = self.peers.drain(..).collect();

        for peer in peers {
            println!("\nTrying peer {}", peer.addr());

            let mut conn = match PeerConnection::connect(
                peer,
                &self.info_hash,
                &client.peer_id,
                self.pieces.len(),
            )
            .await
            {
                Ok(conn) => conn,
                Err((peer, e)) => {
                    eprintln!("Peer {} failed: {e}", peer.addr());
                    continue;
                }
            };

            if let Err(e) = conn.receive_initial_messages().await {
                eprintln!("Failed to receive initial messages: {e}");
                continue;
            }

            if let Err(e) = conn.send_interested().await {
                eprintln!("Failed to send interested: {e}");
                continue;
            }

            match timeout(Duration::from_secs(30), self.download_from(&mut conn)).await {
                Ok(Ok(())) => break,
                Ok(Err(e)) => {
                    eprintln!("Download failed: {e}");
                    continue;
                }
                Err(_) => {
                    eprintln!("Download timed out");
                    continue;
                }
            }
        }

        Ok(())
    }

    pub async fn download_from(&mut self, conn: &mut PeerConnection) -> Result<()> {
        loop {
            let Some(piece_idx) = self.pick_piece(conn.peer().bitfield()) else {
                break;
            };

            let data = conn.download_piece(piece_idx, self.piece_length).await?;
            self.verify_and_store(piece_idx, &data)?;
        }

        Ok(())
    }

    fn pick_piece(&self, bitfield: &BitField) -> Option<usize> {
        (0..self.pieces.len()).find(|&piece| bitfield.has_piece(piece) && !self.have[piece])
    }

    fn verify_and_store(&mut self, piece_idx: usize, data: &[u8]) -> Result<()> {
        let piece_hash: [u8; 20] = Sha1::digest(data).into();
        if piece_hash != self.pieces[piece_idx] {
            return Err(anyhow!("Piece {} hash mismatch", piece_idx));
        }

        self.have[piece_idx] = true;
        self.downloaded += data.len() as u64;
        self.left = self.left.saturating_sub(data.len() as u64);

        println!("Verified piece {}", piece_idx);
        Ok(())
    }
}

impl TryFrom<Object> for Torrent {
    type Error = anyhow::Error;

    fn try_from(object: Object) -> Result<Self> {
        let dict = match object.object_type() {
            ObjectType::Dictionary(d) => d,
            _ => return Err(anyhow!("Top level object is not a dictionary")),
        };

        let announce = Tracker::new(
            Url::parse(&extract_str(&dict, b"announce")?).context("invalid announce URL")?,
        );
        let announce_list = extract_announce_list(&dict)?;
        let comment = extract_str(&dict, b"comment")?;
        let created_by = extract_str(&dict, b"created by")?;
        let creation_date = u64::try_from(extract_num(&dict, b"creation date")?)
            .map_err(|_| anyhow!("creation date is negative or too large"))?;

        let info_obj = extract_dict(&dict, b"info")?;
        let name = extract_str(&info_obj, b"name")?;
        let length = u64::try_from(extract_num(&info_obj, b"length")?)
            .map_err(|_| anyhow!("length is negative or too large"))?;
        let piece_length = u64::try_from(extract_num(&info_obj, b"piece length")?)
            .map_err(|_| anyhow!("piece length is negative or too large"))?;
        let pieces = extract_pieces(&info_obj)?;
        let info_hash = compute_info_hash(&dict)?;
        let have = vec![false; pieces.len()];

        Ok(Torrent {
            tracker: announce,
            announce_list,
            comment,
            created_by,
            creation_date,
            name,
            length,
            piece_length,
            pieces,
            info_hash,
            downloaded: 0,
            left: length,
            uploaded: 0,
            peers: Vec::new(),
            have,
        })
    }
}

fn extract_announce_list(dict: &BTreeMap<Vec<u8>, Object>) -> Result<Vec<Vec<Tracker>>> {
    let tiers = extract_list(dict, b"announce-list")?;

    let mut announce_list = Vec::new();

    for tier in tiers {
        let mut trackers = Vec::new();

        let list = match tier.object_type() {
            ObjectType::List(l) => l,
            _ => {
                return Err(anyhow!(
                    "Expected key {} to be of type {}",
                    "announce-list",
                    "list"
                ));
            }
        };

        for obj in list {
            let bytes = match obj.object_type() {
                ObjectType::ByteArray(b) => b,
                _ => {
                    return Err(anyhow!(
                        "Expected key {} to be {}",
                        "announce-list",
                        "byte string",
                    )
                    .into());
                }
            };

            let url = String::from_utf8(bytes.to_vec())?;

            trackers.push(Tracker::new(Url::parse(&url)?));
        }

        announce_list.push(trackers);
    }

    Ok(announce_list)
}

fn compute_info_hash(dict: &BTreeMap<Vec<u8>, Object>) -> Result<[u8; 20]> {
    let info_parsed = dict
        .get(b"info".as_slice())
        .ok_or(anyhow!("Missing key info"))?;
    Ok(Sha1::digest(&info_parsed.bytes()).into())
}

fn extract_pieces(info_dict: &BTreeMap<Vec<u8>, Object>) -> Result<Vec<[u8; 20]>> {
    let arr = extract_byte_array(info_dict, b"pieces")?;
    chunk_array::<20>(&arr)
}

fn chunk_array<const N: usize>(data: &[u8]) -> Result<Vec<[u8; N]>> {
    if data.len() % N != 0 {
        return Err(anyhow!("Length {} is not a mupliple of {}", data.len(), N));
    }

    let mut result = Vec::with_capacity(data.len() / N);

    for chunk in data.chunks(N) {
        let mut arr = [0u8; N];
        arr.copy_from_slice(chunk);
        result.push(arr);
    }

    Ok(result)
}
