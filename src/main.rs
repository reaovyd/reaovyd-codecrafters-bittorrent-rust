use anyhow::Result;
use bittorrent_starter_rust::{
    handshake::Handshake,
    torrent::{from_file, FileType},
    tracker::{Compact, QueryStringBuilder, TrackerResponse},
    util, HANDSHAKE_SIZE,
};
use clap::Parser;
use reqwest::Client;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    match cli.commands {
        cli::Commands::Decode { bencoded_value } => {
            let decoded = util::decode_bencoded_value(&bencoded_value)?;
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
                &info.info_hash()?,
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
                    let response = client.execute(req).await?;
                    let bytes = response.bytes().await?;
                    let response = TrackerResponse::from_bytes(&bytes)?;

                    for peer in response.peers() {
                        println!("{}", peer);
                    }
                }
                Err(err) => {
                    eprintln!("{}", err);
                }
            }
        }
        cli::Commands::Handshake {
            torrent_file,
            peer_addr,
        } => {
            let (_, info) = from_file(torrent_file)?;
            let mut request_body = Handshake::new(&info.info_hash()?, b"00112233445566778899");
            let stream = TcpStream::connect(peer_addr).await?;
            let (mut reader, mut writer) = stream.into_split();
            let body = &mut request_body.clone().into_bytes()[..];
            writer.write_all(body).await?;
            let mut buf = [0; HANDSHAKE_SIZE];
            reader.read_exact(&mut buf).await?;
            let handshake = Handshake::from_bytes(&buf)?;
            assert_eq!(request_body, handshake);
            let peer_id = handshake.peer_id();
            println!("Peer ID: {}", hex::encode(peer_id));
        }
        cli::Commands::DownloadPiece {
            torrent_file,
            piece_num,
            out_file,
        } => {
            // When a peer finishes downloading a piece and checks that the hash matches,
            // it announces that it has that piece to all of its peers.
            //
            // Each peer connections two bits of state on both of the ends:
            // 1. Choked or Not choked (in which it is unchoking)
            //  - Choking is a notification that no data will be sent until unchoking
            //    occurs
            // 2. Interested or Not Interested
            // Data transfer ONLY takes place if and only if
            // 1. The side sending data is unchoking
            // 2. The side receiving data is interested
        }
    };
    Ok(())
}
