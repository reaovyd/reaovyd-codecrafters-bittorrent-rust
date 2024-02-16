use std::{
    path::{Path, PathBuf},
    str,
};

use reqwest::Url;
use serde_bencode::value::Value;
use thiserror::Error;

/// A bencoded dictionary that represents metadata for the actual torrent file data
#[derive(Debug, Clone)]
pub struct MetaInfo {
    /// URL of the central tracker to communicate with
    announce: Url,
    /// Name of the path to the file or directory that it should save the file as
    ///
    /// In the single file case, it is the name of a file and a name of a directory in the multi
    /// file case
    name: PathBuf,
    /// Maps to the number of bytes in each piece the file is split into
    piece_length: u64,
    /// Maps to a string whose length is a multiple of 20. Each 20 bytes is a hash value created by
    /// the SHA-1 hashing algorithm and represents a unique ID of a piece.
    pieces: Vec<[u8; 20]>,
    file_type: FileType,
}

#[derive(Debug, Clone)]
pub enum FileType {
    SingleFile(u64),
    MultiFile { files: Vec<FileInfo> },
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    length: u64,
    path: PathBuf,
}

impl FileInfo {
    fn new(length: u64, path: Vec<String>) -> Self {
        let mut buf = PathBuf::new();
        for path in path {
            buf.push(path);
        }
        Self { length, path: buf }
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl MetaInfo {
    fn new(
        announce: Url,
        name: impl AsRef<Path>,
        piece_length: u64,
        pieces: Vec<[u8; 20]>,
        file_type: FileType,
    ) -> Self {
        let name = PathBuf::from(name.as_ref());
        Self {
            announce,
            name,
            piece_length,
            pieces,
            file_type,
        }
    }

    pub fn read_from_file(file: impl AsRef<Path>) -> Result<Self, MetaInfoError> {
        let bytes =
            std::fs::read(file).map_err(|err| MetaInfoError::Deserialization(err.to_string()))?;
        Self::read_from_bytes(bytes)
    }

    pub fn read_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, MetaInfoError> {
        Self::try_from(bytes.as_ref())
    }

    pub fn announce(&self) -> &Url {
        &self.announce
    }

    pub fn name(&self) -> &PathBuf {
        &self.name
    }

    pub fn piece_length(&self) -> u64 {
        self.piece_length
    }

    pub fn pieces(&self) -> &Vec<[u8; 20]> {
        &self.pieces
    }

    pub fn file_type(&self) -> &FileType {
        &self.file_type
    }
}

impl TryFrom<&[u8]> for MetaInfo {
    type Error = MetaInfoError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let value = serde_bencode::from_bytes::<Value>(value)
            .map_err(|err| MetaInfoError::Deserialization(err.to_string()))?;
        if let Value::Dict(map) = value {
            if let Value::Bytes(announce) = map
                .get("announce".as_bytes())
                .ok_or(MetaInfoError::MissingField("announce".to_string()))?
            {
                let url = str::from_utf8(announce)
                    .map_err(|utf8_err| MetaInfoError::Deserialization(utf8_err.to_string()))?;
                let announce = Url::parse(url)
                    .map_err(|url_err| MetaInfoError::Deserialization(url_err.to_string()))?;
                if let Value::Dict(info) = map
                    .get("info".as_bytes())
                    .ok_or(MetaInfoError::MissingField("info".to_string()))?
                {
                    let name = {
                        if let Value::Bytes(name) = info
                            .get("name".as_bytes())
                            .ok_or(MetaInfoError::MissingField("name".to_string()))?
                        {
                            Ok(String::from_utf8(name.to_vec()).map_err(|utf8_err| {
                                MetaInfoError::Deserialization(utf8_err.to_string())
                            })?)
                        } else {
                            Err(MetaInfoError::Deserialization(
                                "`name` did not deserialize into a string/bytes".to_string(),
                            ))
                        }
                    }?;
                    let piece_length = {
                        if let Value::Int(piece_length) = info
                            .get("piece length".as_bytes())
                            .ok_or(MetaInfoError::MissingField("piece length".to_string()))?
                        {
                            let piece_length = u64::try_from(*piece_length)
                                .map_err(|err| MetaInfoError::Deserialization(err.to_string()))?;
                            Ok(piece_length)
                        } else {
                            Err(MetaInfoError::Deserialization(
                                "`piece_length` did not deserialize into an integer".to_string(),
                            ))
                        }
                    }?;
                    let pieces = {
                        if let Value::Bytes(pieces) = info
                            .get("pieces".as_bytes())
                            .ok_or(MetaInfoError::MissingField("pieces".to_string()))?
                        {
                            if pieces.len() % 20 != 0 || pieces.is_empty() {
                                Err(MetaInfoError::Deserialization(
                                    "Length of pieces string was not a multiple of 20!".to_string(),
                                ))
                            } else {
                                let chunks = pieces.chunks_exact(20);
                                let mut chunks_res = Vec::new();
                                for chunk in chunks {
                                    let chunk: [u8; 20] =
                                        chunk[0..20].try_into().map_err(|_| {
                                            MetaInfoError::Deserialization(
                                                "Chunk failed to parse into a [u8; 20]".to_string(),
                                            )
                                        })?;
                                    chunks_res.push(chunk);
                                }
                                Ok(chunks_res)
                            }
                        } else {
                            Err(MetaInfoError::Deserialization(
                                "`pieces` did not deserialize into bytes".to_string(),
                            ))
                        }
                    }?;
                    let file_type = {
                        match (info.get("length".as_bytes()), info.get("files".as_bytes())) {
                            (None, None) => {
                                // TODO
                                Err(MetaInfoError::Deserialization(
                                    "Found neither `length` nor `file`".to_string(),
                                ))
                            }
                            (Some(_), Some(_)) => Err(MetaInfoError::Deserialization(
                                "Found both `length` and `file`!".to_string(),
                            )),
                            (None, Some(files)) => {
                                if let Value::List(files) = files {
                                    let mut fileinfos = Vec::new();
                                    for file in files {
                                        if let Value::Dict(file) = file {
                                            let length = {
                                                if let Value::Int(length) = file
                                                    .get("length".as_bytes())
                                                    .ok_or(MetaInfoError::MissingField(
                                                        "length".to_string(),
                                                    ))?
                                                {
                                                    let length =
                                                        u64::try_from(*length).map_err(|err| {
                                                            MetaInfoError::Deserialization(
                                                                err.to_string(),
                                                            )
                                                        })?;
                                                    Ok(length)
                                                } else {
                                                    Err(MetaInfoError::Deserialization(
                                                        "`length` did not deserialize into an integer".to_owned(),
                                                    ))
                                                }
                                            }?;
                                            let path = {
                                                if let Value::List(path) = file
                                                    .get("path".as_bytes())
                                                    .ok_or(MetaInfoError::MissingField(
                                                        "path".to_string(),
                                                    ))?
                                                {
                                                    let mut vec = Vec::new();
                                                    for sub in path {
                                                        if let Value::Bytes(sub) = sub {
                                                            let sub = str::from_utf8(sub).map_err(
                                                                |err| {
                                                                    MetaInfoError::Deserialization(
                                                                        err.to_string(),
                                                                    )
                                                                },
                                                            )?;
                                                            vec.push(sub.to_owned());
                                                        } else {
                                                            return Err(MetaInfoError::Deserialization(
                                                                "`path` did not deserialize into a string/bytes"
                                                                    .to_string(),
                                                            ));
                                                        }
                                                    }
                                                    Ok(vec)
                                                } else {
                                                    Err(MetaInfoError::Deserialization(
                                                        "`path` did not deserialize into a list"
                                                            .to_string(),
                                                    ))
                                                }
                                            }?;
                                            fileinfos.push(FileInfo::new(length, path));
                                        } else {
                                            return Err(MetaInfoError::Deserialization(
                                                "`file` did not deserialize into a dictionary"
                                                    .to_string(),
                                            ));
                                        }
                                    }
                                    Ok(FileType::MultiFile { files: fileinfos })
                                } else {
                                    Err(MetaInfoError::Deserialization(
                                        "`files` did not deserialize into a list".to_string(),
                                    ))
                                }
                            }
                            (Some(length), None) => {
                                if let Value::Int(length) = length {
                                    let length = u64::try_from(*length).map_err(|err| {
                                        MetaInfoError::Deserialization(err.to_string())
                                    })?;
                                    Ok(FileType::SingleFile(length))
                                } else {
                                    Err(MetaInfoError::Deserialization(
                                        "`length` did not deserialize into an integer".to_owned(),
                                    ))
                                }
                            }
                        }
                    }?;
                    Ok(MetaInfo::new(
                        announce,
                        name,
                        piece_length,
                        pieces,
                        file_type,
                    ))
                } else {
                    Err(MetaInfoError::Deserialization(
                        "`info` key has been found not to deserialize into a dictionary"
                            .to_string(),
                    ))
                }
            } else {
                Err(MetaInfoError::Deserialization(
                    "`announce` key has been found not to deserialize into bytes/a string"
                        .to_string(),
                ))
            }
        } else {
            Err(MetaInfoError::Deserialization(
                "Initial Metainfo bytes format was not a map!".to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetaInfoError {
    #[error("Deserialization failed: {0}")]
    Deserialization(String),
    #[error("Missing MetaInfoField: {0}")]
    MissingField(String),
}
