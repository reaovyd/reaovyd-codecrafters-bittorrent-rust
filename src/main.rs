// Available if you need it!
use bittorrent_starter_rust::{
    peers::tracker::{Compact, QueryStringBuilder},
    torrent::{FileType, MetaInfo},
    util,
};
use clap::Parser;
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
                        println!("{:?}", res);
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
