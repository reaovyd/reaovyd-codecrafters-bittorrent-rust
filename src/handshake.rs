use thiserror::Error;
use tokio::{
    io,
    net::{TcpStream, ToSocketAddrs},
};

use super::INFO_HASH_SIZE;
use std::io::{Cursor, Read};

use crate::{ParseError, PEER_ID_SIZE};

pub const LENGTH_BYTE_SIZE: usize = 1;
pub const HANDSHAKE_LENGTH_SIZE: usize = 19;
pub const RESERVED_SIZE: usize = 8;
pub const PROTOCOL_STRING: [u8; HANDSHAKE_LENGTH_SIZE] = *b"BitTorrent protocol";
pub const HANDSHAKE_SIZE: usize =
    LENGTH_BYTE_SIZE + HANDSHAKE_LENGTH_SIZE + RESERVED_SIZE + INFO_HASH_SIZE + PEER_ID_SIZE;

pub async fn connect<A: ToSocketAddrs>(
    peer: A,
    info_hash: &[u8; INFO_HASH_SIZE],
    peer_id: &[u8; PEER_ID_SIZE],
) -> Result<(TcpStream, [u8; PEER_ID_SIZE]), HandshakeError> {
    let self_hand = Handshake::new(info_hash, peer_id);
    let mut stream = TcpStream::connect(peer)
        .await
        .map_err(|err| HandshakeError::Connection(err.to_string()))?;
    let body = self_hand.as_bytes();
    io::AsyncWriteExt::write_all(&mut stream, &body)
        .await
        .map_err(|err| HandshakeError::Connection(err.to_string()))?;

    let mut buf = [0; HANDSHAKE_SIZE];
    io::AsyncReadExt::read_exact(&mut stream, &mut buf)
        .await
        .map_err(|err| HandshakeError::Connection(err.to_string()))?;
    let peer_hand =
        Handshake::from_bytes(&buf).map_err(|err| HandshakeError::Connection(err.to_string()))?;
    if self_hand != peer_hand {
        return Err(HandshakeError::Connection(
            "Handshake could not be validated since hands did not compromise!".to_owned(),
        ));
    }

    Ok((stream, peer_hand.peer_id))
}

// TODO: If the receiving side's peer id doesn't match the one the initiating side expects, it severs the connection.
#[derive(Debug, Clone)]
pub struct Handshake {
    length: u8,
    protocol: [u8; HANDSHAKE_LENGTH_SIZE],
    reserved: [u8; RESERVED_SIZE],
    infohash: [u8; INFO_HASH_SIZE],
    peer_id: [u8; PEER_ID_SIZE],
}

impl PartialEq for Handshake {
    fn eq(&self, other: &Self) -> bool {
        self.length == other.length
            && self.protocol == other.protocol
            && self.infohash == other.infohash
    }
}

impl Eq for Handshake {}

impl Handshake {
    pub fn length(&self) -> &u8 {
        &self.length
    }

    pub fn protocol(&self) -> &[u8; HANDSHAKE_LENGTH_SIZE] {
        &self.protocol
    }

    pub fn reserved(&self) -> &[u8; RESERVED_SIZE] {
        &self.reserved
    }

    pub fn infohash(&self) -> &[u8; INFO_HASH_SIZE] {
        &self.infohash
    }

    pub fn peer_id(&self) -> &[u8; PEER_ID_SIZE] {
        &self.peer_id
    }

    pub fn new(infohash: &[u8; 20], peer_id: &[u8; 20]) -> Self {
        Self {
            length: HANDSHAKE_LENGTH_SIZE as u8,
            protocol: PROTOCOL_STRING,
            reserved: [0u8; RESERVED_SIZE],
            infohash: *infohash,
            peer_id: *peer_id,
        }
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Handshake, ParseError> {
        let mut cursor = Cursor::new(bytes);
        let mut length = [0u8];
        cursor
            .read_exact(&mut length)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
        let length = length[0];
        if usize::from(length) != HANDSHAKE_LENGTH_SIZE {
            return Err(ParseError::Deserialization(format!(
                "`length` parsed did not equal to required handshake length. Found {}",
                length
            )));
        }
        let mut protocol = [0u8; HANDSHAKE_LENGTH_SIZE];
        cursor
            .read_exact(&mut protocol)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
        if protocol != PROTOCOL_STRING {
            return Err(ParseError::Deserialization(format!(
                "protocol parsed did not equal to required handshake length. Found {:?}",
                protocol
            )));
        }
        // We don't care for these reserved bytes
        // for now
        let mut reserved = [0; 8];
        cursor
            .read_exact(&mut reserved)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
        let mut infohash = [0; 20];
        cursor
            .read_exact(&mut infohash)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
        let mut peer_id = [0; 20];
        cursor
            .read_exact(&mut peer_id)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
        Ok(Self {
            length,
            protocol,
            reserved,
            infohash,
            peer_id,
        })
    }
    pub fn as_bytes(&self) -> [u8; HANDSHAKE_SIZE] {
        let mut serialized = vec![*self.length()];
        serialized.extend_from_slice(&self.protocol);
        serialized.extend_from_slice(&self.reserved);
        serialized.extend_from_slice(&self.infohash);
        serialized.extend_from_slice(&self.peer_id);
        <[u8; HANDSHAKE_SIZE]>::try_from(serialized).expect("Should be handshake sized...")
    }
}

impl TryFrom<&[u8]> for Handshake {
    type Error = ParseError;
    fn try_from(value: &[u8]) -> Result<Handshake, ParseError> {
        Handshake::from_bytes(value)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HandshakeError {
    #[error("Error connecting to the peer: {0}")]
    Connection(String),
}
