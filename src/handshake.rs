use super::INFO_HASH_SIZE;
use std::io::{Cursor, Read};

use crate::{ParseError, PEER_ID_SIZE};

pub const LENGTH_BYTE_SIZE: usize = 1;
pub const HANDSHAKE_LENGTH_SIZE: usize = 19;
pub const RESERVED_SIZE: usize = 8;
pub const PROTOCOL_STRING: [u8; HANDSHAKE_LENGTH_SIZE] = *b"BitTorrent protocol";
pub const HANDSHAKE_SIZE: usize =
    LENGTH_BYTE_SIZE + HANDSHAKE_LENGTH_SIZE + RESERVED_SIZE + INFO_HASH_SIZE + PEER_ID_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    length: u8,
    protocol: [u8; HANDSHAKE_LENGTH_SIZE],
    reserved: [u8; RESERVED_SIZE],
    infohash: [u8; INFO_HASH_SIZE],
    peer_id: [u8; PEER_ID_SIZE],
}

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
    pub fn into_bytes(self) -> Vec<u8> {
        let mut serialized = vec![self.length];
        let mut add_bytes = |bytes: &[u8]| {
            for byte in bytes {
                serialized.push(*byte)
            }
        };
        add_bytes(&self.protocol[..]);
        add_bytes(&self.reserved[..]);
        add_bytes(&self.infohash[..]);
        add_bytes(&self.peer_id[..]);
        serialized
    }
}

impl TryFrom<&[u8]> for Handshake {
    type Error = ParseError;
    fn try_from(value: &[u8]) -> Result<Handshake, ParseError> {
        Handshake::from_bytes(value)
    }
}

impl From<Handshake> for Vec<u8> {
    fn from(value: Handshake) -> Self {
        value.into_bytes()
    }
}
