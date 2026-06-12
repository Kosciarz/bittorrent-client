use std::{cmp::min, fmt::Write, sync::Arc};

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use tokio::sync::mpsc;

use crate::torrent_info::TorrentInfo;

pub enum ProgressEvent {
    PieceCompleted,
}

pub struct ProgressDisplayer {
    info: Arc<TorrentInfo>,

    piece_rx: mpsc::Receiver<ProgressEvent>,
}

impl ProgressDisplayer {
    pub fn new(info: Arc<TorrentInfo>, piece_rx: mpsc::Receiver<ProgressEvent>) -> Self {
        Self { info, piece_rx }
    }

    pub async fn run(&mut self) {
        let mut downloaded = 0;

        let bar = ProgressBar::new(self.info.total_length);
        bar.set_style(
            ProgressStyle::with_template(
                "Downloading [{wide_bar:.cyan/blue}] {percent_completed} {bytes}/{total_bytes} ETA {eta}",
            )
            .unwrap()
            .progress_chars("##-")
            .with_key("percent_completed", |state: &ProgressState, w: &mut dyn Write| {
                    write!(w, "{:.1}%", state.fraction() * 100.0).unwrap();
                }),
        );

        while let Some(event) = self.piece_rx.recv().await {
            match event {
                ProgressEvent::PieceCompleted => {
                    let new = min(
                        downloaded + self.info.piece_length as u64,
                        self.info.total_length,
                    );
                    downloaded = new;
                    bar.set_position(new);
                }
            }
        }

        bar.finish();
    }
}
