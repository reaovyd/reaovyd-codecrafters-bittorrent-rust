use anyhow::{anyhow, Result};
use std::{net::SocketAddrV4, path::Path};

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
    pub async fn download(&self, torrent_file: impl AsRef<Path>) {
        // 1. Need to spawn a downloader for that torrent_file
    }
    // pub fn new(
    //     client: Client,
    //     peer_id: &[u8; PEER_ID_SIZE],
    //     torrent_file: impl AsRef<Path>,
    // ) -> Result<Self, PeerClientError> {
    //     // TODO: Something from async to sync bridge code since I'm having a TcpListener on
    //     // another port

    //     let port = 6881;
    //     let (mut url, metainfo) =
    //         from_file(torrent_file).map_err(|err| PeerClientError::Connection(err.to_string()))?;
    //     let left = match metainfo.file_type() {
    //         crate::torrent::FileType::SingleFile(len) => *len,
    //         crate::torrent::FileType::MultiFile(_) => {
    //             todo!("Multifile support is not implemented yet!")
    //         }
    //     };
    //     let info_hash = metainfo
    //         .info_hash()
    //         .map_err(|err| PeerClientError::Connection(err.to_string()))?;
    //     let init_query_string =
    //         QueryStringBuilder::new(&info_hash, peer_id, port, 0, 0, left, Compact::Compact)
    //             .build();
    //     url.set_query(Some(&init_query_string));
    //     todo!()
    // }
    // pub async fn discover_new_peer(
    //     &mut self,
    //     uploaded: u64,
    //     downloaded: u64,
    //     left: u64,
    //     compact: Compact,
    // ) -> Result<SocketAddrV4, PeerClientError> {
    //     // TODO: can we avoid cloning here?
    //     let mut url = self.url.clone();
    //     let query_string = QueryStringBuilder::new(
    //         &self.info_hash,
    //         &self.peer_id,
    //         self.listener_port,
    //         uploaded,
    //         downloaded,
    //         left,
    //         compact,
    //     )
    //     .build();
    //     url.set_query(Some(&query_string));
    //     let req = self
    //         .client
    //         .get(url)
    //         .build()
    //         .map_err(|err| PeerClientError::PeerDiscovery(err.to_string()))?;
    //     let resp = self
    //         .client
    //         .execute(req)
    //         .await
    //         .map_err(|err| PeerClientError::PeerDiscovery(err.to_string()))?
    //         .bytes()
    //         .await
    //         .map_err(|err| PeerClientError::PeerDiscovery(err.to_string()))?;
    //     let resp = TrackerResponse::from_bytes(&resp)
    //         .map_err(|err| PeerClientError::PeerDiscovery(err.to_string()))?;
    //     let peer = resp
    //         .peers()
    //         .first()
    //         .ok_or(PeerClientError::PeerDiscovery(
    //             "Couldn't find a peer!".to_owned(),
    //         ))?
    //         .to_owned();
    //     Ok(peer)
    // }
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
    pub async fn download_piece(&mut self, piece_num: usize) -> Result<(), DownloadError> {
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
        for i in 0..=rounds {
            if i == rounds && bytes_left == 0 {
                break;
            }
            let index = u32::try_from(piece_num)
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?
                .to_be_bytes();
            let begin = u32::try_from(i * Downloader::BLK_SIZE)
                .map_err(|err| DownloadError::InvalidPiece {
                    piece_num,
                    reason: err.to_string(),
                })?
                .to_be_bytes();
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
            bytes.extend_from_slice(&index);
            bytes.extend_from_slice(&begin);
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
            println!("{:?}", msg);
        }

        piece.0 = true;
        Ok(())
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
