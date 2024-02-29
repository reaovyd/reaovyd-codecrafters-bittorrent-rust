use anyhow::Result;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

#[derive(Debug)]
pub struct PeerBufferStream {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
}

impl PeerBufferStream {
    pub fn new(reader: OwnedReadHalf, writer: OwnedWriteHalf) -> Self {
        Self { reader, writer }
    }

    pub async fn read_message(&mut self) -> Result<PeerMessage> {
        let length = self.reader.read_u32().await?;
        let id = PeerMessageId::try_from(self.reader.read_u8().await?)?;
        // TODO: think we need to subtract one byte here probably if message length
        // prefix contains the id byte too in its length?
        let mut payload = vec![0; usize::try_from(length - 1)?];
        self.reader.read_exact(&mut payload[..]).await?;
        Ok(PeerMessage {
            length,
            id,
            payload,
        })
    }
    pub async fn write_message(&mut self, id: PeerMessageId, payload: &[u8]) -> Result<()> {
        let byte_len = std::mem::size_of::<PeerMessageId>();
        let payload_len = payload.len();
        let message_prefix_length = u32::try_from(byte_len + payload_len)?;
        let mut buf = message_prefix_length.to_be_bytes().to_vec();
        buf.push(id as u8);
        buf.extend_from_slice(payload);
        self.writer.write_all(&buf).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
    pub length: u32,
    pub id: PeerMessageId,
    pub payload: Vec<u8>,
}

// impl PeerMessage {
//     pub fn from_bytes(bytes: &[u8]) -> Result<Self, PeerParseError> {
//         let mut cursor = Cursor::new(bytes);
//         if cursor.remaining() < size_of::<u32>() {
//             return Err(PeerParseError::Deserialization(
//                 "Not enough bytes to get the message length!".to_owned(),
//             ));
//         }
//         let length = cursor.get_u32();
//         if cursor.remaining() < size_of::<u8>() {
//             return Err(PeerParseError::Deserialization(
//                 "Not enough bytes to get the message ID!".to_owned(),
//             ));
//         }
//         let message_id = PeerMessageId::try_from(cursor.get_u8())?;
//         if cursor.remaining()
//             < usize::try_from(length - 1)
//                 .map_err(|err| PeerParseError::Deserialization(err.to_string()))?
//         {}
//
//         todo!()
//     }
// }

#[derive(Debug, Clone, Error)]
pub enum PeerParseError {
    #[error("Error while trying to deserialize bytes into a peer message: {0}")]
    Deserialization(String),
}
