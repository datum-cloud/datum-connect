//! Command line arguments.
use clap::{Parser, Subcommand};
use lib::Repo;
use std::net::{SocketAddrV4, SocketAddrV6};

/// Datum Agent CLI
#[derive(Parser, Debug)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Listen on a magicsocket and forward incoming connections to the specified
    /// host and port. Every incoming bidi stream is forwarded to a new connection.
    ///
    /// Will print a node ticket on stderr that can be used to connect.
    ///
    /// As far as the magic socket is concerned, this is listening. But it is
    /// connecting to a TCP socket for which you have to specify the host and port.
    Listen(ListenArgs),

    /// Connect to a magicsocket, open a bidi stream, and forward stdin/stdout
    /// to it.
    ///
    /// A node ticket is required to connect.
    ///
    /// As far as the magic socket is concerned, this is connecting. But it is
    /// listening on a TCP socket for which you have to specify the interface and port.
    Connect(ConnectArgs),
}

#[derive(Parser, Debug)]
pub struct ListenArgs {
    #[clap(flatten)]
    pub common: CommonArgs,
}

#[derive(Parser, Debug)]
pub struct ConnectArgs {
    /// The addresses to listen on for incoming tcp connections.
    ///
    /// To listen on all network interfaces, use 0.0.0.0:12345
    #[clap(long)]
    pub addr: String,

    #[clap(flatten)]
    pub common: CommonArgs,
}

#[derive(Parser, Debug)]
pub struct CommonArgs {
    /// The IPv4 address that magicsocket will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    #[clap(long, default_value = None)]
    pub magic_ipv4_addr: Option<SocketAddrV4>,

    /// The IPv6 address that magicsocket will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    #[clap(long, default_value = None)]
    pub magic_ipv6_addr: Option<SocketAddrV6>,

    /// The verbosity level. Repeat to increase verbosity.
    #[clap(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let repo = Repo::open_or_create(Repo::default_location()).await?;

    let args = Args::parse();
    match args.command {
        Commands::Listen(_args) => {
            let node = repo.spawn_listen_node().await?;
            node.listen("connection".to_string()).await.unwrap();
            println!("{}", node.endpoint_id());
            tokio::signal::ctrl_c().await?;
            println!()
        }
        Commands::Connect(args) => {
            let ConnectArgs { addr, .. } = args;
            let node = repo.spawn_connect_node().await?;
            node.connect("connection".to_string(), addr).await.unwrap();
            println!("{}", node.endpoint_id());
            tokio::signal::ctrl_c().await?;
            println!()
        }
    }
    Ok(())
}
