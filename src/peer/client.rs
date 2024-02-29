use anyhow::{anyhow, Result};
use sha1::{Digest, Sha1};
use std::{mem, net::SocketAddrV4, path::Path};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

use reqwest::Client;
use thiserror::Error;

use crate::{
    handshake,
    peer::message::{PeerBufferStream, PeerMessageId},
    torrent::{from_file, FileType, MetaInfo},
    tracker::{discover_peers, Compact},
    INFO_HASH_SIZE, PEER_ID_SIZE,
};

use super::message::PeerMessage;

// NOTE: Currently tightly coupled with the torrent_file
// TODO: Make it decoupled with torrent_file and users of
// the client can provide torrent_files to a method?
#[derive(Debug)]
pub struct PeerClient {
    peer_id: [u8; PEER_ID_SIZE],
    client: Client,
    listener_port: u16,
}

impl PeerClient {
    pub fn new(client: Client, peer_id: [u8; PEER_ID_SIZE]) -> Self {
        Self {
            client,
            peer_id,
            listener_port: 6881,
        }
    }
    pub async fn download(
        &self,
        torrent_file: impl AsRef<Path>,
        out_file: impl AsRef<Path>,
    ) -> Result<()> {
        let mut downloader = Downloader::new(
            &self.client,
            self.listener_port,
            torrent_file,
            &self.peer_id,
            Compact::Compact,
        )
        .await?;
        let pieces_len = downloader.pieces_downloaded.len();
        // TODO: optimize by writing to hard disk since this can store nearly like 5gb in ram lol
        let mut final_output = vec![];
        for piece_idx in 0..pieces_len {
            let bytes = downloader.download_piece(piece_idx).await?;
            final_output.extend(bytes);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(out_file)
            .await?;
        file.write_all(&final_output).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Downloader {
    pieces_downloaded: Vec<(bool, u64)>,
    metainfo: MetaInfo,
    info_hash: [u8; INFO_HASH_SIZE],
    peer_id: [u8; PEER_ID_SIZE],
    peers: Vec<SocketAddrV4>,
}

impl Downloader {
    /// BLK_SIZE = 2^14
    const BLK_SIZE: u64 = 1 << 14;
    pub async fn new(
        client: &Client,
        port: u16,
        torrent_file: impl AsRef<Path>,
        peer_id: &[u8; PEER_ID_SIZE],
        compact: Compact,
    ) -> Result<Downloader> {
        let (url, info) = from_file(torrent_file)?;
        let left = match info.file_type() {
            FileType::SingleFile(left) => *left,
            FileType::MultiFile(_) => todo!("MultiFile support is not implemented yet!"),
        };
        let info_hash = info.info_hash()?;
        let peers = discover_peers(
            client,
            &info_hash,
            url,
            port,
            compact,
            peer_id,
            (0, 0, left),
        )
        .await?;
        let piece_len = info.pieces().len();
        let piece_size = info.piece_length();
        let last_piece_length = left % piece_size;
        let mut pieces_downloaded = vec![(false, piece_size); piece_len];
        let piece_downloaded_len = pieces_downloaded.len();
        if last_piece_length != 0 {
            pieces_downloaded[piece_downloaded_len - 1] = (false, last_piece_length);
        }
        Ok(Self {
            peers,
            info_hash,
            peer_id: *peer_id,
            metainfo: info,
            pieces_downloaded,
        })
    }
    pub async fn download_piece(&mut self, piece_num: usize) -> Result<Vec<u8>, DownloadError> {
        let piece =
            self.pieces_downloaded
                .get_mut(piece_num)
                .ok_or(DownloadError::InvalidPiece {
                    piece_num,
                    reason: "piece num is out of bounds".to_owned(),
                })?;
        if piece.0 {
            return Err(DownloadError::InvalidPiece {
                piece_num,
                reason: "piece has already been downloaded!".to_owned(),
            });
        }
        let peer = self.peers.first().ok_or(DownloadError::InvalidPiece {
            piece_num,
            reason: "No peer found!".to_owned(),
        })?;
        let (stream, _peer) = handshake::connect(peer, &self.info_hash, &self.peer_id)
            .await
            .map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?;
        let (reader, writer) = stream.into_split();
        let mut stream = PeerBufferStream::new(reader, writer);
        verify_incoming_message(&mut stream, PeerMessageId::Bitfield)
            .await
            .map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?;
        stream
            .write_message(PeerMessageId::Interested, &[])
            .await
            .map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?;
        verify_incoming_message(&mut stream, PeerMessageId::Unchoke)
            .await
            .map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?;
        let length = piece.1;
        let rounds = length / Downloader::BLK_SIZE;
        let bytes_left = length % Downloader::BLK_SIZE;
        let mut piece_bytes = vec![];
        // TODO: make this concurrent later?
        for i in 0..=rounds {
            if i == rounds && bytes_left == 0 {
                break;
            }
            let index = u32::try_from(piece_num).map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?;
            let begin = u32::try_from(i * Downloader::BLK_SIZE).map_err(|err| {
                DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                }
            })?;
            let length = u32::try_from({
                if i != rounds {
                    Downloader::BLK_SIZE
                } else {
                    bytes_left
                }
            })
            .map_err(|err| DownloadError::InvalidPiece {
                piece_num,
                reason: err.to_string(),
            })?
            .to_be_bytes();
            let mut bytes = vec![];
            bytes.extend_from_slice(&index.to_be_bytes());
            bytes.extend_from_slice(&begin.to_be_bytes());
            bytes.extend_from_slice(&length);
            stream
                .write_message(PeerMessageId::Request, &bytes)
                .await
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?;
            let msg = verify_incoming_message(&mut stream, PeerMessageId::Piece)
                .await
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?;
            let actual_index = u32::from_be_bytes(
                <[u8; mem::size_of::<u32>()]>::try_from(
                    msg.payload.get(0..mem::size_of::<u32>()).ok_or(
                        DownloadError::InvalidPiece {
                            piece_num,
                            reason: "Couldn't get resp index!".to_owned(),
                        },
                    )?,
                )
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?,
            );
            if actual_index != index {
                return Err(DownloadError::InvalidPiece {
                    piece_num,
                    reason: format!(
                        "Did not download the correct index! Got {} but expected {}",
                        actual_index, index
                    ),
                });
            }
            let actual_begin = u32::from_be_bytes(
                <[u8; mem::size_of::<u32>()]>::try_from(
                    msg.payload
                        .get(mem::size_of::<u32>()..(mem::size_of::<u32>() * 2))
                        .ok_or(DownloadError::InvalidPiece {
                            piece_num,
                            reason: "Couldn't get resp index!".to_owned(),
                        })?,
                )
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?,
            );
            if actual_begin != begin {
                return Err(DownloadError::InvalidPiece {
                    piece_num,
                    reason: format!(
                        "Did not download the correct begin bytes! Got {} but expected {}",
                        actual_begin, begin
                    ),
                });
            }
            piece_bytes.extend(msg.payload.get((mem::size_of::<u32>() * 2)..).ok_or(
                DownloadError::InvalidPiece {
                    piece_num,
                    reason: "Couldn't get the actual block!".to_owned(),
                },
            )?);
        }
        let bytes = Sha1::digest(&piece_bytes);
        let actual_bytes = <[u8; INFO_HASH_SIZE]>::from(bytes);
        let expected_bytes = self.metainfo.pieces()[piece_num];
        if actual_bytes != expected_bytes {
            return Err(DownloadError::InvalidPiece {
                piece_num,
                reason: format!(
                    "SHA-1 Hashes did not match. Got {}, but expected {}",
                    hex::encode(actual_bytes),
                    hex::encode(expected_bytes)
                ),
            });
        }
        piece.0 = true;
        Ok(piece_bytes)
    }
}

async fn verify_incoming_message(
    stream: &mut PeerBufferStream,
    expected: PeerMessageId,
) -> Result<PeerMessage> {
    let message = stream.read_message().await?;
    if message.id == expected {
        Ok(message)
    } else {
        Err(anyhow!(
            "Did not get a {} message; got {}",
            stringify!(expected),
            stringify!(message.id)
        ))
    }
}

#[derive(Debug, Error)]
pub enum PeerClientError {
    #[error("Error connecting this peer to the p2p network: {0}")]
    Connection(String),
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("Error downloading piece {piece_num:?}: {reason:?}")]
    InvalidPiece { piece_num: usize, reason: String },
}
