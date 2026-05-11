use anyhow::Result;

mod bencode;
mod bitfield;
mod client;
mod file_writer;
mod peer;
mod peer_manager;
mod piece;
mod piece_picker;
mod piece_validator;
mod stats_manager;
mod torrent_info;
mod torrent_session;
mod tracker;
mod tracker_manager;

#[tokio::main]
async fn main() -> Result<()> {
    let client = client::Client::new();
    client.run().await
}
