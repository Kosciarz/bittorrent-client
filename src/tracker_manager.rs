use std::{collections::HashMap, net::SocketAddr, sync::Arc};

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
    trackers: Vec<Tracker>,
    cancellation_token: CancellationToken,
    peers: HashMap<SocketAddr, Peer>,

    stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
    peer_tx: mpsc::Sender<Peer>,
}

impl TrackerManager {
    pub fn new(
        info: Arc<TorrentInfo>,
        client: Client,
        cancellation_token: CancellationToken,
        stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
        peer_tx: mpsc::Sender<Peer>,
    ) -> Self {
        let mut trackers = Vec::new();

        match Tracker::new(info.announce.clone()) {
            Ok(t) => trackers.push(t),
            Err(e) => eprintln!("invalid announce URL {}: {}", info.announce, e),
        };

        for tier in &info.announce_list {
            for url in tier {
                match Tracker::new(url.clone()) {
                    Ok(t) => trackers.push(t),
                    Err(e) => eprintln!("invalid tracker URL {}: {}", url, e),
                }
            }
        }

        if trackers.is_empty() {
            eprintln!("no usable trackers found");
        }

        Self {
            info,
            client,
            trackers,
            cancellation_token,
            peers: HashMap::new(),
            stats_manager_command_tx,
            peer_tx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(self.trackers[0].interval());

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

                    for tracker in &self.trackers {
                        match tracker.announce(
                            &self.info.info_hash,
                            &self.client.peer_id,
                            self.client.port,
                            &AnnounceStats { uploaded: stats.uploaded, downloaded: stats.downloaded, left: stats.left },
                        ).await {
                            Ok(addrs) => {
                                for addr in addrs {
                                    if !self.peers.contains_key(&addr) {
                                        self.peers.insert(addr, Peer { addr });
                                        let _ = self.peer_tx.send(self.peers[&addr].clone()).await;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Tracker {} failed: {}", tracker.url(), e);
                                continue;
                            }
                        }
                    }

                }
                _ = self.cancellation_token.cancelled() => {
                    break Ok(());
                }
            }
        }
    }
}
