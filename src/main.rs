use anyhow::{Context, Result};
use bittorrent_starter_rust::{
    handshake::{self},
    peer::client::Downloader,
    torrent::{from_file, FileType},
    tracker::{self, Compact},
    util,
};
use clap::Parser;
use reqwest::Client;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};
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
            let (url, info) =
                from_file(torrent_file).context("Failed to parse metainfo from file")?;
            let length = {
                if let FileType::SingleFile(length) = info.file_type() {
                    *length
                } else {
                    panic!("Expected single type files only!")
                }
            };
            let info_hash = info.info_hash().context("Failed to calculate info hash!")?;
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
            let (url, info) =
                from_file(torrent_file).context("Failed to parse metainfo from file")?;
            let left_length = {
                match info.file_type() {
                    FileType::SingleFile(length) => *length,
                    FileType::MultiFile(_) => {
                        todo!("Multifile torrent support has not been implemented yet!")
                    }
                }
            };
            let peers = tracker::discover_peers(
                &client,
                &info.info_hash()?,
                url,
                6881,
                Compact::Compact,
                b"00112233445566778899",
                (0, 0, left_length),
            )
            .await?;
            for peer in peers {
                println!("{}", peer);
            }
        }
        cli::Commands::Handshake {
            torrent_file,
            peer_addr,
        } => {
            let (_, info) = from_file(torrent_file)?;
            let (_, peer_id) =
                handshake::connect(peer_addr, &info.info_hash()?, b"00112233445566778899").await?;
            println!("Peer ID: {}", hex::encode(peer_id));
        }
        cli::Commands::DownloadPiece {
            torrent_file,
            piece_num,
            out_file,
        } => {
            let client = Client::new();
            let peer_id = b"00112233445566778899";
            let mut downloader =
                Downloader::new(&client, 6881, torrent_file, peer_id, Compact::Compact).await?;
            let bytes = downloader.download_piece(piece_num).await?;
            let mut file = OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(out_file)
                .await?;
            file.write_all(&bytes).await?;
        }
    };
    Ok(())
}
