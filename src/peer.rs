use std::{io::Cursor, mem::size_of};

use bytes::Buf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerMessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl TryFrom<u8> for PeerMessageId {
    type Error = PeerParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let value = match value {
            0 => PeerMessageId::Choke,
            1 => PeerMessageId::Unchoke,
            2 => PeerMessageId::Interested,
            3 => PeerMessageId::NotInterested,
            4 => PeerMessageId::Have,
            5 => PeerMessageId::Bitfield,
            6 => PeerMessageId::Request,
            7 => PeerMessageId::Piece,
            8 => PeerMessageId::Cancel,
            _ => {
                return Err(PeerParseError::Deserialization(format!(
                    "Message {value} is not defined in this implementation!"
                )))
            }
        };
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerMessage {
    length: u32,
    id: PeerMessageId,
    payload: Vec<u8>,
}

impl PeerMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PeerParseError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.remaining() < size_of::<u32>() {
            return Err(PeerParseError::Deserialization(
                "Not enough bytes to get the message length!".to_owned(),
            ));
        }
        let length = cursor.get_u32();
        if cursor.remaining() < size_of::<u8>() {
            return Err(PeerParseError::Deserialization(
                "Not enough bytes to get the message ID!".to_owned(),
            ));
        }
        let message_id = PeerMessageId::try_from(cursor.get_u8())?;
        if cursor.remaining() < length - 1 {}

        todo!()
    }
}

#[derive(Debug, Clone, Error)]
pub enum PeerParseError {
    #[error("Error while trying to deserialize bytes into a peer message: {0}")]
    Deserialization(String),
}
