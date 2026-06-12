use tokio::sync::{mpsc, oneshot};

use crate::{bitfield::BitField, progress_displayer::ProgressEvent, torrent_session::TorrentEvent};

#[derive(Debug, Clone, PartialEq)]
pub enum PieceState {
    Missing,
    InProgress,
    Completed,
}

#[derive(Debug, Clone)]
pub enum PieceEvent {
    Completed { piece_index: usize },
    HashMismatch { piece_index: usize },
    DownloadFailed { piece_index: usize },
}

#[derive(Debug)]
pub enum PiecePickerCommand {
    RequestPiece {
        bitfield: BitField,
        response_tx: oneshot::Sender<Option<usize>>,
    },
}

#[derive(Debug)]
pub struct PiecePicker {
    states: Vec<PieceState>,

    piece_event_rx: mpsc::Receiver<PieceEvent>,
    piece_picker_command_rx: mpsc::Receiver<PiecePickerCommand>,
    torrent_event_tx: mpsc::Sender<TorrentEvent>,
    progress_event_tx: mpsc::Sender<ProgressEvent>,
}

impl PiecePicker {
    pub fn new(
        num_pieces: usize,
        piece_event_rx: mpsc::Receiver<PieceEvent>,
        piece_picker_command_rx: mpsc::Receiver<PiecePickerCommand>,
        torrent_event_tx: mpsc::Sender<TorrentEvent>,
        progress_event_tx: mpsc::Sender<ProgressEvent>,
    ) -> Self {
        Self {
            states: vec![PieceState::Missing; num_pieces],
            piece_event_rx,
            piece_picker_command_rx,
            torrent_event_tx,
            progress_event_tx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select!(
                Some(event) = self.piece_event_rx.recv() =>  {
                    match event {
                        PieceEvent::HashMismatch { piece_index }
                        | PieceEvent::DownloadFailed { piece_index } => {
                            self.mark_as_failed(piece_index);
                        }
                        PieceEvent::Completed { piece_index } => {
                            self.mark_as_completed(piece_index);

                            if self.is_finished() {
                                let _ = self.torrent_event_tx.send(TorrentEvent::Completed).await;
                                return;
                            }

                            let _ = self.progress_event_tx.send(ProgressEvent::PieceCompleted).await;
                        },
                    }
                }
                Some(cmd) = self.piece_picker_command_rx.recv() => {
                    match cmd {
                        PiecePickerCommand::RequestPiece { bitfield, response_tx } =>{
                            let idx = self.claim_piece(&bitfield);
                            let _ = response_tx.send(idx);
                        },
                    }
                }
            )
        }
    }

    pub fn claim_piece(&mut self, bitfield: &BitField) -> Option<usize> {
        let idx = self.states.iter().enumerate().find_map(|(i, state)| {
            (bitfield.has_piece(i) && *state == PieceState::Missing).then_some(i)
        })?;

        self.states[idx] = PieceState::InProgress;
        Some(idx)
    }

    fn mark_as_completed(&mut self, index: usize) {
        if self.states[index] == PieceState::InProgress {
            self.states[index] = PieceState::Completed;
        }
    }

    fn mark_as_failed(&mut self, index: usize) {
        if self.states[index] == PieceState::InProgress {
            self.states[index] = PieceState::Missing;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.states
            .iter()
            .all(|state| *state == PieceState::Completed)
    }
}
