use sha1::{Digest, Sha1};
use tokio::sync::mpsc;

use crate::torrent::{Piece, PieceState};

#[derive(Debug)]
pub struct PieceAssembler {
    piece_hashes: Vec<[u8; 20]>,

    piece_rx: mpsc::Receiver<Piece>,
    file_tx: mpsc::Sender<Piece>,
}

impl PieceAssembler {
    pub fn new(piece_hashes: Vec<[u8; 20]>, piece_rx: mpsc::Receiver<Piece>, file_tx: mpsc::Sender<Piece>) -> Self {
        Self { piece_hashes, piece_rx, file_tx }
    }

    pub async fn run(&mut self) {
        while let Some(piece) = self.piece_rx.recv().await {
            if self.verify_piece(&piece) {
                let _ = self.file_tx.send(piece).await;
            }
        }
    }

    fn verify_piece(&self, piece: &Piece) -> bool {
        let data = match &piece.state {
            PieceState::Downloaded { data } => data,
            _ => unreachable!("piece must be in Downloaded state here"),
        };

        let piece_hash: [u8; 20] = Sha1::digest(&data).into();
        piece_hash != self.piece_hashes[piece.index]
    }
}
