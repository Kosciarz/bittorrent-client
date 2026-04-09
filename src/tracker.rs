use std::{
    error::Error,
    time::Duration,
};

use crate::{bencode::{
    ObjectType, decode_object,
    object::{extract_byte_array, extract_num},
}, peer::Peer};

pub struct Tracker {
    url: String,
    interval: Duration,
    peers: Vec<Peer>,
}

#[derive(Debug)]
pub struct AnnounceInfo<'a> {
    info_hash: &'a [u8],
    peer_id: &'a str,
    peer_port: u16,
    downloaded: u64,
    left: u64,
    uploaded: u64,
}

impl<'a> AnnounceInfo<'a> {
    pub fn new(
        info_hash: &'a [u8],
        peer_id: &'a str,
        peer_port: u16,
        downloaded: u64,
        left: u64,
        uploaded: u64,
    ) -> Self {
        Self {
            info_hash,
            peer_id,
            peer_port,
            downloaded,
            left,
            uploaded,
        }
    }
}

impl Tracker {
    pub fn new(url: String) -> Self {
        Self {
            url,
            interval: Duration::ZERO,
            peers: Vec::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    pub fn peers(&self) -> &Vec<Peer> {
        &self.peers
    }

    pub fn update(&mut self, announce_info: &AnnounceInfo) -> Result<(), Box<dyn Error>> {
        let url_encoded_info_hash = urlencoding::encode_binary(announce_info.info_hash);

        let url = format!(
            "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1",
            self.url,
            url_encoded_info_hash,
            announce_info.peer_id,
            announce_info.peer_port,
            announce_info.uploaded,
            announce_info.downloaded,
            announce_info.left,
        );

        let res = reqwest::blocking::get(url)?.bytes()?;
        let object = decode_object(&res);

        match object.object_type() {
            ObjectType::Dictionary(d) => {
                let complete = extract_num(&d, b"complete")?;
                let incomplete = extract_num(&d, b"incomplete")?;
                let interval = extract_num(&d, b"interval")?;
                self.set_interval(Duration::from_secs(interval as u64));

                let peers = extract_byte_array(&d, b"peers")?;

                let peer = Peer::new_from_bytes(&peers);

                println!(
                    "Complete: {}\nIncomplete: {}\nInterval: {}\nPeer: {:?}",
                    complete, incomplete, interval, peer
                );
            }
            _ => panic!("Expected a dictionary"),
        }

        Ok(())
    }
}

impl std::fmt::Debug for Tracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tracker")
            .field("address", &self.url)
            .finish()
    }
}
