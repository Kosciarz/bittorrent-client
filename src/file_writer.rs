use std::sync::Arc;

use tokio::{
    fs::{self},
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
    info: Arc<TorrentInfo>,

    files: Vec<fs::File>,
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
        let mut files = Vec::with_capacity(info.files.len());

        for file_item in &info.files {
            if let Some(parent) = file_item.path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let file_handle = fs::File::options()
                .create(true)
                .write(true)
                .read(true)
                .open(&file_item.path)
                .await?;

            file_handle.set_len(file_item.length).await?;

            files.push(file_handle);
        }

        Ok(Self {
            info,
            files,
            completed_piece_rx,
            piece_event_tx,
            stats_manager_command_tx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(completed) = self.completed_piece_rx.recv().await {
            let total_offset = (completed.index as u64) * (self.info.piece_length as u64);
            let mut remaining_data = completed.data.as_slice();
            let mut current_offset = total_offset;

            for i in 0..self.files.len() {
                let file = &self.info.files[i];

                if current_offset >= file.offset + file.length {
                    continue;
                }

                let file_offset = current_offset - self.info.files[i].offset;
                let space_in_file = file.length - file_offset;
                let write_len = remaining_data.len().min(space_in_file as usize);

                self.files[i].seek(io::SeekFrom::Start(file_offset)).await?;
                self.files[i]
                    .write_all(&remaining_data[..write_len])
                    .await?;

                remaining_data = &remaining_data[write_len..];
                current_offset += write_len as u64;

                if remaining_data.is_empty() {
                    break;
                }
            }

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
