// Available if you need it!
use bittorrent_starter_rust::torrent::{FileType, MetaInfo};
use clap::Parser;
mod cli;
mod util;

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
            println!("Tracker URL: {}", s);
            println!("Length: {}", length);
            println!("Info Hash: {}", info_hash);
        }
        cli::Commands::Peers { torrent_file } => todo!(),
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
