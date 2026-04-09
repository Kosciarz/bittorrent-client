use std::{env, error::Error, path::Path};

use crate::torrent::Torrent;

mod bencode;
mod torrent;
mod tracker;
mod peer;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Invalid argument count");
    }

    let path = args[1].to_string();
    let path = Path::new(&path);
    let mut torrent = Torrent::load_from_file(path)?;
    torrent.update_trackers()?;

    Ok(())
}
