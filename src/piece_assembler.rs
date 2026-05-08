use tokio::sync::mpsc;

use crate::piece::{ActivePiece, CompletedPiece};

#[derive(Debug)]
pub struct PieceAssembler {
    piece_hashes: Vec<[u8; 20]>,

    piece_rx: mpsc::Receiver<ActivePiece>,
    file_tx: mpsc::Sender<CompletedPiece>,
}

impl PieceAssembler {
    pub fn new(
        piece_hashes: Vec<[u8; 20]>,
        piece_rx: mpsc::Receiver<ActivePiece>,
        file_tx: mpsc::Sender<CompletedPiece>,
    ) -> Self {
        Self {
            piece_hashes,
            piece_rx,
            file_tx,
        }
    }

    pub async fn run(&mut self) {
        while let Some(mut piece) = self.piece_rx.recv().await {
            if piece.verify(&self.piece_hashes[piece.index]) {
                let _ = self
                    .file_tx
                    .send(CompletedPiece {
                        index: piece.index,
                        data: std::mem::take(&mut piece.data),
                    })
                    .await;
            }
        }
    }
}
