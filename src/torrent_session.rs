use std::{collections::HashSet, sync::Arc};

use anyhow::{Context, Result, bail};
use tokio::{
    sync::{
        Mutex,
        mpsc::{self, Sender},
        oneshot,
    },
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    client::Client,
    file_writer::FileWriter,
    peer::{Peer, PeerConnection},
    piece::{ActivePiece, CompletedPiece},
    piece_assembler::PieceValidator,
    piece_picker::{PieceEvent, PiecePicker, PiecePickerCommand},
    stats_manager::{StatsManager, StatsManagerCommand},
    torrent_info::TorrentInfo,
    tracker::{AnnounceStats, Tracker},
};

#[derive(Debug)]
pub enum TorrentEvent {
    Completed,
}

#[derive(Debug, Clone)]
pub struct TorrentSession {
    pub info: Arc<TorrentInfo>,

    tracker: Arc<Tracker>,
    tracker_list: Vec<Vec<Arc<Tracker>>>,

    piece_event_tx: mpsc::Sender<PieceEvent>,
    piece_picker_event_tx: mpsc::Sender<PiecePickerCommand>,
    active_piece_tx: mpsc::Sender<ActivePiece>,
    torrent_event_rx: Arc<Mutex<mpsc::Receiver<TorrentEvent>>>,
    stats_manager_command_tx: mpsc::Sender<StatsManagerCommand>,
}

impl TorrentSession {
    pub async fn new(info: Arc<TorrentInfo>) -> Result<Self> {
        let (stats_manager_command_tx, stats_manager_command_rx) = mpsc::channel(32);
        let mut stats_manager = StatsManager::new(info.length, stats_manager_command_rx);
        tokio::spawn(async move { stats_manager.run().await });

        let (torrent_event_tx, torrent_event_rx) = mpsc::channel(10);

        let (piece_event_tx, piece_event_rx) = mpsc::channel(256);

        let (piece_picker_event_tx, piece_picker_event_rx) = mpsc::channel(32);
        let mut piece_picker = PiecePicker::new(
            info.pieces.len(),
            piece_event_rx,
            piece_picker_event_rx,
            torrent_event_tx,
        );
        tokio::spawn(async move { piece_picker.run().await });

        let (completed_piece_tx, completed_piece_rx) = mpsc::channel::<CompletedPiece>(32);
        let mut file_writer = FileWriter::new(
            Arc::clone(&info),
            completed_piece_rx,
            piece_event_tx.clone(),
            stats_manager_command_tx.clone(),
        )
        .await?;
        tokio::spawn(async move { file_writer.run().await });

        let (piece_tx, piece_rx) = mpsc::channel::<ActivePiece>(32);
        let mut piece_assembler = PieceValidator::new(
            info.piece_hashes(),
            piece_rx,
            completed_piece_tx.clone(),
            piece_event_tx.clone(),
        );
        tokio::spawn(async move { piece_assembler.run().await });

        let tracker = Arc::new(Tracker::new(info.announce.clone()));

        let mut tracker_list = Vec::new();
        for tier in &info.announce_list {
            let mut trackers = Vec::new();

            for tracker in tier {
                trackers.push(Arc::new(Tracker::new(tracker.clone())));
            }

            tracker_list.push(trackers);
        }

        Ok(Self {
            info,
            tracker,
            tracker_list,
            piece_event_tx,
            piece_picker_event_tx,
            active_piece_tx: piece_tx,
            torrent_event_rx: Arc::new(Mutex::new(torrent_event_rx)),
            stats_manager_command_tx,
        })
    }

    pub async fn run(&self, client: &Client) -> Result<()> {
        let (peer_tx, peer_rx) = mpsc::channel::<Vec<Peer>>(1);

        let cancel = CancellationToken::new();

        let announce_task = tokio::spawn({
            let torrent = self.clone();
            let client = client.clone();
            let cancel = cancel.clone();

            async move { torrent.run_announce_loop(peer_tx, &client, cancel).await }
        });

        let download_task = tokio::spawn({
            let mut torrent = self.clone();
            let client = client.clone();
            let cancel = cancel.clone();

            async move { torrent.run_download_loop(peer_rx, &client, cancel).await }
        });

        let (announce_result, download_result) = tokio::join!(announce_task, download_task);

        announce_result??;
        download_result??;

        Ok(())
    }

    async fn run_announce_loop(
        &self,
        peer_tx: Sender<Vec<Peer>>,
        client: &Client,
        cancel: CancellationToken,
    ) -> Result<()> {
        let mut addr_set = HashSet::new();

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
                            &client.peer_id,
                            client.port,
                            &AnnounceStats { uploaded: stats.uploaded, downloaded: stats.downloaded, left: stats.left },
                        )
                        .await?;

                    let mut peers = Vec::new();
                    for addr in addrs {
                        if addr_set.insert(addr) {
                            peers.push(Peer { addr });
                        }
                    }

                    if !peers.is_empty() {
                        peer_tx.send(peers).await?;
                    }
                },
                _ = cancel.cancelled() => return Ok(())
            }
        }
    }

    async fn run_download_loop(
        &mut self,
        mut peer_rx: mpsc::Receiver<Vec<Peer>>,
        client: &Client,
        cancel: CancellationToken,
    ) -> Result<()> {
        let mut join_set = JoinSet::new();

        loop {
            let mut torrent_event_rx = self.torrent_event_rx.lock().await;

            tokio::select! {
                Some(peers) = peer_rx.recv() => {
                    self.process_peers(peers, &mut join_set, client);
                }
                Some(res) = join_set.join_next() => {
                    match res {
                        Ok(Ok(())) => {},
                        Ok(Err(e)) => eprintln!("peer connection failed: {e}"),
                        Err(e) => eprintln!("peer task panicked: {e}"),
                    }
                }
                Some(event) = torrent_event_rx.recv() => {
                    match event {
                        TorrentEvent::Completed => {
                            cancel.cancel();
                            join_set.abort_all();
                            return Ok(());
                        },
                    }
                }
                else => {
                    cancel.cancel();
                    bail!("ran out of peers before download completed");
                },
            }
        }
    }

    fn process_peers(&self, peers: Vec<Peer>, join_set: &mut JoinSet<Result<()>>, client: &Client) {
        for peer in peers {
            let info = Arc::clone(&self.info);
            let client = client.clone();
            let active_piece_tx = self.active_piece_tx.clone();
            let piece_picker_event_tx = self.piece_picker_event_tx.clone();
            let piece_event_tx = self.piece_event_tx.clone();

            join_set.spawn(async move {
                let addr = peer.addr;
                let mut conn = PeerConnection::connect(
                    info,
                    peer,
                    &client.peer_id,
                    active_piece_tx,
                    piece_picker_event_tx,
                    piece_event_tx,
                )
                .await
                .context(format!("peer {addr} failed"))?;

                conn.send_interested()
                    .await
                    .context("failed to send interested")?;

                conn.wait_until_ready()
                    .await
                    .context("failed to receive initial messages")?;

                conn.run().await.context("download failed")
            });
        }
    }
}
