use thiserror::Error;

pub mod torrent;
pub mod tracker;
pub mod util;

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
