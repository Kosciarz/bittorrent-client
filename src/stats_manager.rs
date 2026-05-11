use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub downloaded: u64,
    pub left: u64,
    pub uploaded: u64,
}

#[derive(Debug)]
pub enum StatsManagerCommand {
    UpdateDownloaded { bytes: usize },
    Fetch { response_tx: oneshot::Sender<Stats> },
}

#[derive(Debug)]
pub struct StatsManager {
    stats: Stats,
    stats_manager_command_rx: mpsc::Receiver<StatsManagerCommand>,
}

impl StatsManager {
    pub fn new(
        torrent_length: u64,
        stats_manager_command_rx: mpsc::Receiver<StatsManagerCommand>,
    ) -> Self {
        Self {
            stats: Stats {
                downloaded: 0,
                left: torrent_length,
                uploaded: 0,
            },
            stats_manager_command_rx,
        }
    }

    pub async fn run(&mut self) {
        while let Some(cmd) = self.stats_manager_command_rx.recv().await {
            match cmd {
                StatsManagerCommand::UpdateDownloaded { bytes } => {
                    self.stats.downloaded += bytes as u64;
                    self.stats.left -= bytes as u64;
                }
                StatsManagerCommand::Fetch {
                    response_tx: response,
                } => {
                    let _ = response.send(self.stats);
                }
            }
        }
    }
}
