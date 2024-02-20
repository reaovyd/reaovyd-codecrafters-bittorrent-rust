use std::{
    fmt::Display,
    net::{Ipv4Addr, SocketAddrV4},
};

use serde_bencode::value::Value;
use thiserror::Error;

use crate::ParseError;

macro_rules! add_query_string {
    ($queries: ident, $key:ident, $val:expr) => {{
        let query = stringify!($key);
        let s = $val;
        $queries.push(format!("{query}={}", s));
    }};
}

const TRACKER_RESPONSE_PEER_SIZE: usize = 6;

#[derive(Debug, Clone)]
pub struct TrackerResponse {
    interval: u64,
    peers: Vec<SocketAddrV4>,
}

impl TrackerResponse {
    pub fn interval(&self) -> &u64 {
        &self.interval
    }

    pub fn peers(&self) -> &Vec<SocketAddrV4> {
        &self.peers
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        if let Value::Dict(res) = serde_bencode::from_bytes::<Value>(bytes)
            .map_err(|err| ParseError::Deserialization(err.to_string()))?
        {
            let interval = {
                if let Value::Int(interval) =
                    res.get("interval".as_bytes())
                        .ok_or(ParseError::MissingField(
                            "`interval` was not found!".to_owned(),
                        ))?
                {
                    Ok(u64::try_from(*interval)
                        .map_err(|err| ParseError::Deserialization(err.to_string()))?)
                } else {
                    Err(ParseError::Deserialization(
                        "`interval` did not deserialize into an integer".to_owned(),
                    ))
                }
            }?;
            let peers = {
                if let Value::Bytes(bytes) = res.get("peers".as_bytes()).ok_or(
                    ParseError::MissingField("`peers` was not found!".to_owned()),
                )? {
                    if bytes.len() % TRACKER_RESPONSE_PEER_SIZE != 0 {
                        Err(ParseError::Deserialization("`bytes` length was not a multiple of 6 which is necessary for collecting each peer <ip>:<port>".to_owned()))
                    } else {
                        Ok(bytes
                            .chunks_exact(TRACKER_RESPONSE_PEER_SIZE)
                            .map(|chunk| {
                                let chunk = <[u8; TRACKER_RESPONSE_PEER_SIZE]>::try_from(chunk)
                                    .expect("Must necessarily be 6 bytes");
                                let ip = <[u8; 4]>::try_from(&chunk[0..4])
                                    .expect("Must necessarily be 4 bytes");
                                let port = <[u8; 2]>::try_from(&chunk[4..])
                                    .expect("Must necessarily be 2 bytes");
                                let port = (port[0] as u16) << 8 | port[1] as u16;
                                SocketAddrV4::new(Ipv4Addr::from(ip), port)
                            })
                            .collect::<Vec<SocketAddrV4>>())
                    }
                } else {
                    Err(ParseError::Deserialization(
                        "`peers` did not deserialize into bytes".to_owned(),
                    ))
                }
            }?;
            Ok(TrackerResponse { interval, peers })
        } else {
            Err(ParseError::Deserialization(
                "Bytes did not deserialize into a dictionary".to_owned(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryStringBuilder {
    info_hash: String,
    peer_id: String,
    ip: Option<Ipv4Addr>,
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    event: Option<Event>,
    compact: Compact,
}

impl QueryStringBuilder {
    pub fn new(
        info_hash: &[u8; 20],
        peer_id: &[u8; 20],
        port: u16,
        uploaded: u64,
        downloaded: u64,
        left: u64,
        compact: Compact,
    ) -> Self {
        Self {
            info_hash: urlencode_bytes(info_hash),
            peer_id: urlencode_bytes(peer_id),
            ip: None,
            port,
            uploaded,
            downloaded,
            left,
            event: None,
            compact,
        }
    }

    pub fn with_ip(self, ip: impl Into<Ipv4Addr>) -> Self {
        let mut s = self;
        s.ip = Some(ip.into());
        s
    }

    pub fn with_event(self, event: Event) -> Self {
        let mut s = self;
        s.event = Some(event);
        s
    }

    pub fn build(self) -> String {
        let mut queries = Vec::new();
        add_query_string!(queries, info_hash, self.info_hash);
        add_query_string!(queries, peer_id, self.peer_id);
        if let Some(ip) = self.ip {
            add_query_string!(queries, ip, ip.to_string());
        }
        add_query_string!(queries, port, self.port);
        add_query_string!(queries, uploaded, self.uploaded);
        add_query_string!(queries, downloaded, self.downloaded);
        add_query_string!(queries, left, self.left);
        if let Some(event) = self.event {
            add_query_string!(queries, event, event);
        }
        add_query_string!(queries, compact, self.compact);

        queries.join("&")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Empty,
    Started,
    Completed,
    Stopped,
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let event = match self {
            Event::Empty => "empty",
            Event::Started => "started",
            Event::Completed => "completed",
            Event::Stopped => "stopped",
        };
        write!(f, "{}", event)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compact {
    NotCompact,
    Compact,
}

impl From<Compact> for u8 {
    fn from(value: Compact) -> Self {
        Self::from(&value)
    }
}

impl From<&Compact> for u8 {
    fn from(value: &Compact) -> Self {
        match value {
            Compact::NotCompact => 0,
            Compact::Compact => 1,
        }
    }
}

impl Display for Compact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", u8::from(self))
    }
}

fn urlencode_bytes(bytes: &[u8]) -> String {
    let mut res = String::new();
    for byte in bytes {
        match byte {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'.' | b'~' | b'-' | b'_' => {
                res.push(*byte as char);
            }
            _ => {
                res.push('%');
                let byte = hex::encode([*byte]);
                res.push_str(&byte);
            }
        };
    }
    res
}

#[cfg(test)]
mod tests {
    use crate::torrent::from_file;

    use super::QueryStringBuilder;

    #[test]
    pub fn test_1() {
        let mut metainfo = from_file("sample.torrent").unwrap();
        let query = QueryStringBuilder::new(
            &metainfo.1.info_hash().unwrap(),
            b"00112233445566778899",
            6881,
            0,
            0,
            0,
            super::Compact::Compact,
        )
        .build();
    }
}
