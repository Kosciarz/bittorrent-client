use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    client::Client,
    peer::Peer,
    stats_manager::StatsManagerCommand,
    torrent_info::TorrentInfo,
    tracker::{AnnounceStats, Tracker},
};

#[derive(Debug)]
pub struct TrackerManager {
    info: Arc<TorrentInfo>,
    client: Client,
    tracker: Tracker,
    tracker_list: Vec<Vec<Tracker>>,
    cancellation_token: CancellationToken,

    stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
    peer_tx: mpsc::Sender<Vec<Peer>>,
}

impl TrackerManager {
    pub fn new(
        info: Arc<TorrentInfo>,
        client: Client,
        cancellation_token: CancellationToken,
        stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
        peer_tx: mpsc::Sender<Vec<Peer>>,
    ) -> Self {
        let tracker = Tracker::new(info.announce.clone());

        let mut tracker_list = Vec::new();
        for tier in &info.announce_list {
            let mut trackers = Vec::new();

            for tracker in tier {
                trackers.push(Tracker::new(tracker.clone()));
            }

            tracker_list.push(trackers);
        }

        Self {
            info,
            client,
            tracker,
            tracker_list,
            cancellation_token,
            stats_manager_command_tx,
            peer_tx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(self.tracker.interval());

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let (tx, rx) = oneshot::channel();

                    self.stats_manager_command_tx.send(
                        StatsManagerCommand::Fetch {
                            response_tx: tx
                        }
                    ).await?;

                    let stats = rx.await?;

                    let addrs = self
                        .tracker
                        .announce(
                            &self.info.info_hash,
                            &self.client.peer_id,
                            self.client.port,
                            &AnnounceStats { uploaded: stats.uploaded, downloaded: stats.downloaded, left: stats.left },
                        )
                        .await?;

                    let peers: Vec<Peer> = addrs.iter().map(|addr| Peer {addr: *addr}).collect();

                    if !peers.is_empty() {
                        self.peer_tx.send(peers).await?;
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    break Ok(());
                }
            }
        }
    }
}
