use std::sync::Arc;

use tokio::{
    fs::File,
    io::{self, AsyncSeekExt, AsyncWriteExt},
    sync::mpsc,
};

use anyhow::Result;

use crate::{
    piece::CompletedPiece, piece_picker::PieceEvent, stats_manager::StatsManagerCommand,
    torrent_info::TorrentInfo,
};

#[derive(Debug)]
pub struct FileWriter {
    piece_length: u32,

    file: File,
    completed_piece_rx: mpsc::Receiver<CompletedPiece>,
    piece_event_tx: mpsc::Sender<PieceEvent>,
    stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
}

impl FileWriter {
    pub async fn new(
        info: Arc<TorrentInfo>,
        completed_piece_rx: mpsc::Receiver<CompletedPiece>,
        piece_event_tx: mpsc::Sender<PieceEvent>,
        stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
    ) -> Result<Self> {
        let file = File::options()
            .create(true)
            .write(true)
            .read(true)
            .open(info.name.clone())
            .await?;

        file.set_len(info.length).await?;

        Ok(Self {
            piece_length: info.piece_length,
            file,
            completed_piece_rx,
            piece_event_tx,
            stats_manager_command_tx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(completed) = self.completed_piece_rx.recv().await {
            let offset = (completed.index as u64) * (self.piece_length as u64);
            self.file.seek(io::SeekFrom::Start(offset)).await?;

            self.file.write_all(&completed.data).await?;

            let _ = self
                .piece_event_tx
                .send(PieceEvent::Completed {
                    piece_index: completed.index,
                })
                .await;

            let _ = self
                .stats_manager_command_tx
                .send(StatsManagerCommand::UpdateDownloaded {
                    bytes: completed.data.len(),
                })
                .await;

            println!("Downloaded piece {}", completed.index);
        }

        Ok(())
    }
}
