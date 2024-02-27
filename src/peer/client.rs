use reqwest::{Client, Url};
use thiserror::Error;

use crate::PEER_ID_SIZE;

#[derive(Debug)]
pub struct PeerClient {
    peer_id: [u8; PEER_ID_SIZE],
    client: Client,
    tracker_url: Url,
}

impl PeerClient {
    pub fn new(client: Client, peer_id: [u8; PEER_ID_SIZE], tracker_url: Url) {}
}

#[derive(Debug, Error)]
pub enum PeerClientError {
    #[error("Error connecting this peer to the p2p network!")]
    Connection,
}
