use std::{
    path::{Path, PathBuf},
    str,
};

use reqwest::Url;
use serde::{Deserialize, Serialize, Serializer};
use serde_bencode::value::Value;
use sha1::{Digest, Sha1};
use thiserror::Error;

use crate::ParseError;

type UrlMetaInfo = (Url, MetaInfo);

#[inline]
pub fn from_file(file: impl AsRef<Path>) -> Result<UrlMetaInfo, ParseError> {
    let bytes = std::fs::read(file).map_err(|err| ParseError::Deserialization(err.to_string()))?;
    from_bytes(bytes)
}

#[inline]
pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<UrlMetaInfo, ParseError> {
    let value = serde_bencode::from_bytes::<Value>(bytes.as_ref())
        .map_err(|err| ParseError::Deserialization(err.to_string()))?;
    if let Value::Dict(map) = value {
        if let Value::Bytes(announce) = map
            .get("announce".as_bytes())
            .ok_or(ParseError::MissingField("announce".to_string()))?
        {
            let url = str::from_utf8(announce)
                .map_err(|utf8_err| ParseError::Deserialization(utf8_err.to_string()))?;
            let announce = Url::parse(url)
                .map_err(|url_err| ParseError::Deserialization(url_err.to_string()))?;
            if let Value::Dict(info) = map
                .get("info".as_bytes())
                .ok_or(ParseError::MissingField("info".to_string()))?
            {
                let name = {
                    if let Value::Bytes(name) = info
                        .get("name".as_bytes())
                        .ok_or(ParseError::MissingField("name".to_string()))?
                    {
                        Ok(String::from_utf8(name.to_vec()).map_err(|utf8_err| {
                            ParseError::Deserialization(utf8_err.to_string())
                        })?)
                    } else {
                        Err(ParseError::Deserialization(
                            "`name` did not deserialize into a string/bytes".to_string(),
                        ))
                    }
                }?;
                let piece_length = {
                    if let Value::Int(piece_length) = info
                        .get("piece length".as_bytes())
                        .ok_or(ParseError::MissingField("piece length".to_string()))?
                    {
                        let piece_length = u64::try_from(*piece_length)
                            .map_err(|err| ParseError::Deserialization(err.to_string()))?;
                        Ok(piece_length)
                    } else {
                        Err(ParseError::Deserialization(
                            "`piece_length` did not deserialize into an integer".to_string(),
                        ))
                    }
                }?;
                let pieces = {
                    if let Value::Bytes(pieces) = info
                        .get("pieces".as_bytes())
                        .ok_or(ParseError::MissingField("pieces".to_string()))?
                    {
                        if pieces.len() % 20 != 0 || pieces.is_empty() {
                            Err(ParseError::Deserialization(
                                "Length of pieces string was not a multiple of 20!".to_string(),
                            ))
                        } else {
                            let chunks = pieces.chunks_exact(20);
                            let mut chunks_res = Vec::new();
                            for chunk in chunks {
                                let chunk: [u8; 20] = chunk[0..20].try_into().map_err(|_| {
                                    ParseError::Deserialization(
                                        "Chunk failed to parse into a [u8; 20]".to_string(),
                                    )
                                })?;
                                chunks_res.push(chunk);
                            }
                            Ok(chunks_res)
                        }
                    } else {
                        Err(ParseError::Deserialization(
                            "`pieces` did not deserialize into bytes".to_string(),
                        ))
                    }
                }?;
                let file_type = {
                    match (info.get("length".as_bytes()), info.get("files".as_bytes())) {
                        (None, None) => Err(ParseError::Deserialization(
                            "Found neither `length` nor `file`".to_string(),
                        )),
                        (Some(_), Some(_)) => Err(ParseError::Deserialization(
                            "Found both `length` and `file`!".to_string(),
                        )),
                        (None, Some(files)) => {
                            if let Value::List(files) = files {
                                let mut fileinfos = Vec::new();
                                for file in files {
                                    if let Value::Dict(file) = file {
                                        let length = {
                                            if let Value::Int(length) =
                                                file.get("length".as_bytes()).ok_or(
                                                    ParseError::MissingField("length".to_string()),
                                                )?
                                            {
                                                let length =
                                                    u64::try_from(*length).map_err(|err| {
                                                        ParseError::Deserialization(err.to_string())
                                                    })?;
                                                Ok(length)
                                            } else {
                                                Err(ParseError::Deserialization(
                                                    "`length` did not deserialize into an integer"
                                                        .to_owned(),
                                                ))
                                            }
                                        }?;
                                        let path = {
                                            if let Value::List(path) =
                                                file.get("path".as_bytes()).ok_or(
                                                    ParseError::MissingField("path".to_string()),
                                                )?
                                            {
                                                if path.is_empty() {
                                                    return Err(ParseError::Deserialization(
                                                        "Empty path!".to_owned(),
                                                    ));
                                                }
                                                let mut vec = Vec::new();
                                                for sub in path {
                                                    if let Value::Bytes(sub) = sub {
                                                        let sub =
                                                            str::from_utf8(sub).map_err(|err| {
                                                                ParseError::Deserialization(
                                                                    err.to_string(),
                                                                )
                                                            })?;
                                                        vec.push(sub.to_owned());
                                                    } else {
                                                        return Err(ParseError::Deserialization(
                                                                "`path` did not deserialize into a string/bytes"
                                                                    .to_string(),
                                                            ));
                                                    }
                                                }
                                                Ok(vec)
                                            } else {
                                                Err(ParseError::Deserialization(
                                                    "`path` did not deserialize into a list"
                                                        .to_string(),
                                                ))
                                            }
                                        }?;
                                        fileinfos.push(FileInfo::new(length, path));
                                    } else {
                                        return Err(ParseError::Deserialization(
                                            "`file` did not deserialize into a dictionary"
                                                .to_string(),
                                        ));
                                    }
                                }
                                Ok(FileType::MultiFile(fileinfos))
                            } else {
                                Err(ParseError::Deserialization(
                                    "`files` did not deserialize into a list".to_string(),
                                ))
                            }
                        }
                        (Some(length), None) => {
                            if let Value::Int(length) = length {
                                let length = u64::try_from(*length)
                                    .map_err(|err| ParseError::Deserialization(err.to_string()))?;
                                Ok(FileType::SingleFile(length))
                            } else {
                                Err(ParseError::Deserialization(
                                    "`length` did not deserialize into an integer".to_owned(),
                                ))
                            }
                        }
                    }
                }?;
                Ok((
                    announce,
                    MetaInfo::new(name, piece_length, pieces, file_type),
                ))
            } else {
                Err(ParseError::Deserialization(
                    "`info` key has been found not to deserialize into a dictionary".to_string(),
                ))
            }
        } else {
            Err(ParseError::Deserialization(
                "`announce` key has been found not to deserialize into bytes/a string".to_string(),
            ))
        }
    } else {
        Err(ParseError::Deserialization(
            "Initial Metainfo bytes format was not a map!".to_string(),
        ))
    }
}

/// A bencoded dictionary that represents metadata for the actual torrent file data
#[derive(Debug, Clone, Serialize)]
pub struct MetaInfo {
    /// Name of the path to the file or directory that it should save the file as
    ///
    /// In the single file case, it is the name of a file and a name of a directory in the multi
    /// file case
    name: PathBuf,
    /// Maps to the number of bytes in each piece the file is split into
    #[serde(rename(serialize = "piece length"))]
    piece_length: u64,
    /// Maps to a string whose length is a multiple of 20. Each 20 bytes is a hash value created by
    /// the SHA-1 hashing algorithm and represents a unique ID of a piece.
    #[serde(serialize_with = "serialize_pieces")]
    pieces: Vec<[u8; 20]>,
    /// The type of the file that the torrent represents
    ///
    /// Represents either a single file or a multi file torrent in which it will have different
    /// data representations
    #[serde(flatten)]
    file_type: FileType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum FileType {
    #[serde(rename(serialize = "length"))]
    SingleFile(u64),
    #[serde(rename(serialize = "files"))]
    MultiFile(Vec<FileInfo>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileInfo {
    length: u64,
    path: PathBuf,
}

impl FileInfo {
    #[inline]
    fn new(length: u64, path: Vec<String>) -> Self {
        let mut buf = PathBuf::new();
        for path in path {
            buf.push(path);
        }
        Self { length, path: buf }
    }

    #[inline]
    pub fn length(&self) -> u64 {
        self.length
    }

    #[inline]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl MetaInfo {
    #[inline]
    fn new(
        name: impl AsRef<Path>,
        piece_length: u64,
        pieces: Vec<[u8; 20]>,
        file_type: FileType,
    ) -> Self {
        let name = PathBuf::from(name.as_ref());
        Self {
            name,
            piece_length,
            pieces,
            file_type,
        }
    }

    #[inline]
    pub fn name(&self) -> &PathBuf {
        &self.name
    }

    #[inline]
    pub fn piece_length(&self) -> u64 {
        self.piece_length
    }

    #[inline]
    pub fn pieces(&self) -> &Vec<[u8; 20]> {
        &self.pieces
    }

    #[inline]
    pub fn file_type(&self) -> &FileType {
        &self.file_type
    }

    #[inline]
    pub fn info_hash(&self) -> Result<[u8; 20], MetaInfoError> {
        let bytes = serde_bencode::to_bytes(self)
            .map_err(|err| MetaInfoError::InfoHash(err.to_string()))?;
        let bytes = Sha1::digest(bytes);
        Ok(<[u8; 20]>::from(bytes))
    }
}

/// Error type for MetaInfo
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetaInfoError {
    /// An error that occurs when trying to create an InfoHash from the MetaInfo struct. This error
    /// occurs typically in one place which is when we're attempting to convert the MetaInfo struct
    /// into bytes and then hashing it with SHA-1
    #[error("InfoHash creation failed: {0}")]
    InfoHash(String),
}

fn serialize_pieces<S>(pieces: &[[u8; 20]], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(&pieces.iter().flatten().copied().collect::<Vec<u8>>()[..])
}

#[cfg(test)]
mod tests {
    use crate::torrent::from_file;

    #[test]
    fn test_deserialize_1() {
        let metainfo = from_file("sample.torrent").unwrap();
        assert_eq!(
            "d69f91e6b2ae4c542468d1073a71d4ea13879a7f",
            hex::encode(metainfo.1.info_hash().unwrap())
        );
    }
}
