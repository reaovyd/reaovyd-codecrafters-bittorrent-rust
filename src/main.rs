use bittorrent_starter_rust::{
    torrent::{from_file, FileType},
    tracker::{Compact, QueryStringBuilder, TrackerResponse},
    util,
};
use clap::Parser;
use reqwest::Client;
mod cli;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    match cli.commands {
        cli::Commands::Decode { bencoded_value } => {
            let decoded = util::decode_bencoded_value(&bencoded_value).unwrap();
            println!("{}", decoded);
        }
        cli::Commands::Info { torrent_file } => {
            let (url, info) = from_file(torrent_file).expect("Failed to parse metainfo from file");
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
            println!("Tracker URL: {}", url);
            println!("Length: {}", length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", piece_length);
            println!("Piece Hashes:");
            for piece in pieces {
                println!("{}", hex::encode(piece));
            }
        }
        cli::Commands::Peers { torrent_file } => {
            let client = Client::new();
            let (mut url, info) =
                from_file(torrent_file).expect("Failed to parse metainfo from file");
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
            url.set_query(Some(&query));
            match client.get(url).build() {
                Ok(req) => {
                    // Should be okay to panic here in my opinion
                    let response = client.execute(req).await.unwrap();
                    let bytes = response.bytes().await.unwrap();
                    let response = TrackerResponse::from_bytes(&bytes).unwrap();
                    for peer in response.peers() {
                        println!("{}", peer);
                    }
                }
                Err(err) => {
                    eprintln!("{}", err)
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
