use std::{fmt::Display, net::Ipv4Addr};

use reqwest::Url;
use serde::{Deserialize, Serialize};

macro_rules! add_query_string {
    ($queries: ident, $key:ident, $val:expr) => {{
        let query = stringify!($key);
        let s = $val;
        $queries.push(format!("{query}={}", s));
    }};
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
            add_query_string!(queries, event, event.to_string());
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

impl ToString for Event {
    fn to_string(&self) -> String {
        match self {
            Event::Empty => "empty",
            Event::Started => "started",
            Event::Completed => "completed",
            Event::Stopped => "stopped",
        }
        .to_owned()
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
    use crate::torrent::{from_file, MetaInfo};

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
