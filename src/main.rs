use std::{env, path::Path};

use crate::torrent::Torrent;
use anyhow::Result;

mod bencode;
mod peer;
mod torrent;
mod tracker;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Invalid argument count");
    }

    let path = args[1].to_string();
    let path = Path::new(&path);
    let mut torrent = Torrent::load_from_file(path)?;
    torrent.update_trackers().await?;

    Ok(())
}
