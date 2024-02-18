use std::net::{Ipv4Addr, SocketAddrV4};

// Available if you need it!
use bittorrent_starter_rust::{
    peers::tracker::{Compact, QueryStringBuilder},
    torrent::{FileType, MetaInfo},
    util,
};
use clap::Parser;
use serde_bencode::value::Value;
mod cli;

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let cli = cli::Cli::parse();
    match cli.commands {
        cli::Commands::Decode { bencoded_value } => {
            let decoded = util::decode_bencoded_value(&bencoded_value).unwrap();
            println!("{}", decoded);
        }
        cli::Commands::Info { torrent_file } => {
            let info =
                MetaInfo::read_from_file(torrent_file).expect("Failed to parse metainfo from file");
            let s = info.announce();
            let length = {
                if let FileType::SingleFile(length) = info.file_type() {
                    *length
                } else {
                    panic!("Expected single type files only!")
                }
            };
            let info_hash = info.info_hash().expect("Failed to calculate info hash!");
            let piece_length = info.piece_length();
            let pieces = info.pieces();
            println!("Tracker URL: {}", s);
            println!("Length: {}", length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", piece_length);
            println!("Piece Hashes:");
            for piece in pieces {
                println!("{}", hex::encode(piece));
            }
        }
        cli::Commands::Peers { torrent_file } => {
            let client = reqwest::blocking::Client::new();
            let mut info =
                MetaInfo::read_from_file(torrent_file).expect("Failed to parse metainfo from file");
            let query = QueryStringBuilder::new(
                &info.info_hash().unwrap(),
                b"00112233445566778899",
                6881,
                0,
                0,
                0,
                Compact::Compact,
            )
            .build();
            info.set_announce_query(&query);
            match client.get(info.announce().clone()).build() {
                Ok(req) => match client.execute(req) {
                    Ok(res) => {
                        if let Value::Dict(value) =
                            serde_bencode::from_bytes::<Value>(&res.bytes().unwrap()).unwrap()
                        {
                            if let Value::Bytes(peers) = value.get("peers".as_bytes()).unwrap() {
                                let chunks = peers.chunks_exact(6);
                                for chunk in chunks {
                                    let chunk = <[u8; 6]>::try_from(chunk).unwrap();
                                    let ip =
                                        Ipv4Addr::from(<[u8; 4]>::try_from(&chunk[0..4]).unwrap());
                                    let port = <[u8; 2]>::try_from(&chunk[4..]).unwrap();
                                    let port = (port[0] as u16) << 8 | port[1] as u16;
                                    let res = SocketAddrV4::new(ip, port);
                                    println!("{res}");
                                }
                            } else {
                                eprintln!("peers did not deserialize into bytes");
                            }
                        } else {
                            eprintln!("response did not deserialize into a dict");
                        }
                    }
                    Err(err) => {
                        eprintln!("{}", err);
                    }
                },
                Err(err) => {
                    eprintln!("{}", err);
                }
            }
        }
        cli::Commands::Handshake {
            torrent_file,
            peer_addr,
        } => todo!(),
        cli::Commands::DownloadPiece {
            torrent_file,
            piece_num,
            out_file,
        } => todo!(),
    }
}
