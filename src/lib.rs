use thiserror::Error;

pub mod handshake;
pub mod torrent;
pub mod tracker;
pub mod util;

pub use handshake::HANDSHAKE_LENGTH_SIZE;
pub use handshake::HANDSHAKE_SIZE;
pub use handshake::LENGTH_BYTE_SIZE;
pub use handshake::PROTOCOL_STRING;
pub use handshake::RESERVED_SIZE;
pub use torrent::INFO_HASH_SIZE;
pub use torrent::PIECE_SIZE;
pub use tracker::PEER_ID_SIZE;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    /// A deserialization error that occurs when trying to parse the raw bytes being read into a
    /// MetaInfo struct
    #[error("Deserialization failed: {0}")]
    Deserialization(String),
    /// A missing field error that occurs when a required field is not found in the raw bytes being
    /// deserialized
    #[error("Missing MetaInfoField: {0}")]
    MissingField(String),
}
