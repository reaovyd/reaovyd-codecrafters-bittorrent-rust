use std::{net::SocketAddrV4, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "snake_case")]
pub enum Commands {
    /// Decodes the serialized bencoded value into a printable format
    Decode {
        /// The serialized bencoded value to be decoded and further printed
        bencoded_value: String,
    },
    /// Parses a torrent file and extracts MetaInfo out of it
    Info {
        /// The torrent file that contains the metainfo to be extracted and printed
        torrent_file: PathBuf,
    },
    /// Searches for peers that have the torrent file
    Peers {
        /// The torrent file that is parsed and extracted from to get the information required for
        /// searching for peers to connect to.
        torrent_file: PathBuf,
    },
    /// Sets up a TCP connection with a peer
    Handshake {
        /// The torrent file that is parsed and is extracted to get information required for
        /// connecting with the peer
        torrent_file: PathBuf,
        /// The IP address and port of the peer: <peer_ip>:<peer_port>
        peer_addr: SocketAddrV4,
    },
    /// Downloads a piece
    DownloadPiece {
        /// The torrent file that is parsed
        torrent_file: PathBuf,
        /// The piece of the torrent to download
        piece_num: usize,
        /// The output path location to the piece downloaded
        #[arg(long, short)]
        out_file: PathBuf,
    },
    /// Downloads an entire file from the torrent
    Download {
        torrent_file: PathBuf,
        /// The output path location to the piece downloaded
        #[arg(long, short)]
        out_file: PathBuf,
    },
}
