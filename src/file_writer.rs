use tokio::{
    fs::File,
    io::{self, AsyncSeekExt, AsyncWriteExt},
    sync::mpsc,
};

use crate::torrent::{Piece, PieceState};
use anyhow::Result;

#[derive(Debug)]
pub struct FileWriter {
    file_rx: mpsc::Receiver<Piece>,
    file: File,
}

impl FileWriter {
    pub async fn new(
        file_rx: mpsc::Receiver<Piece>,
        torrent_length: u64,
        name: String,
    ) -> Result<Self> {
        let file = File::options()
            .create(true)
            .write(true)
            .read(true)
            .open(name)
            .await?;

        file.set_len(torrent_length).await?;

        Ok(Self { file_rx, file })
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(piece) = self.file_rx.recv().await {
            self.file
                .seek(io::SeekFrom::Start(
                    piece.index as u64 * piece.length as u64,
                ))
                .await?;

            let data = match piece.state {
                PieceState::Downloaded { data } => data,
                _ => unreachable!("piece must be in Downloaded state here"),
            };

            self.file.write_all(&data).await?;
            self.file.flush().await?;
        }

        Ok(())
    }
}
