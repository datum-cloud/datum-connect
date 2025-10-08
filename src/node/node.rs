//! Command line arguments.
use iroh::{endpoint::Connecting, Endpoint, NodeAddr, PublicKey, SecretKey, Watcher};
pub use iroh_base::ticket::NodeTicket;
use n0_snafu::{Result, ResultExt};
use std::{
    io,
    net::{SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
    str::FromStr,
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    select,
};
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
use {
    std::path::PathBuf,
    tokio::net::{UnixListener, UnixStream},
};

/// The ALPN for dumbpipe.
///
/// It is basically just passing data through 1:1, except that the connecting
/// side will send a fixed size handshake to make sure the stream is created.
pub const ALPN: &[u8] = b"DUMBPIPEV0";

/// The handshake to send when connecting.
///
/// The side that calls open_bi() first must send this handshake, the side that
/// calls accept_bi() must consume it.
pub const HANDSHAKE: [u8; 5] = *b"hello";

#[derive(Debug, Clone)]
pub struct Node {
    inner: Arc<NodeInner>,
}

impl Node {
    pub async fn new() -> Result<Self> {
        let cfg = CommonArgs {
            ipv4_addr: None,
            ipv6_addr: None,
            verbose: 0,
        };
        let secret_key = get_or_create_secret()?;
        let endpoint = create_endpoint(secret_key, &cfg, vec![ALPN.to_vec()]).await?;
        // wait for the endpoint to figure out its address before making a ticket
        endpoint.home_relay().initialized().await;
        let node_addr = endpoint.node_addr().initialized().await;

        Ok(Self {
            inner: Arc::new(NodeInner {
                ep_id: node_addr.node_id,
                endpoint,
            }),
        })
    }

    pub fn endpoint_id(&self) -> String {
        self.inner.ep_id.to_string()
    }

    pub async fn listen_tcp(&self, args: ListenTcpArgs) -> Result<()> {
        listen_tcp(&self.inner.endpoint, args).await
    }

    pub async fn connect_tcp(&self, args: ConnectTcpArgs) -> Result<()> {
        connect_tcp(&self.inner.endpoint, args).await
    }
}

#[derive(Debug)]
struct NodeInner {
    ep_id: PublicKey,
    endpoint: Endpoint,
}

#[derive(Debug)]
pub struct CommonArgs {
    /// The IPv4 address that the endpoint will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    pub ipv4_addr: Option<SocketAddrV4>,

    /// The IPv6 address that the endpoint will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    pub ipv6_addr: Option<SocketAddrV6>,

    /// The verbosity level. Repeat to increase verbosity.
    pub verbose: u8,
}

#[derive(Debug)]
pub struct ListenTcpArgs {
    pub host: String,

    pub common: CommonArgs,
}

#[derive(Debug)]
pub struct ConnectTcpArgs {
    /// The addresses to listen on for incoming tcp connections.
    ///
    /// To listen on all network interfaces, use 0.0.0.0:12345
    pub addr: String,

    /// The node to connect to
    pub ticket: NodeTicket,

    pub common: CommonArgs,
}

/// Copy from a reader to a quinn stream.
///
/// Will send a reset to the other side if the operation is cancelled, and fail
/// with an error.
///
/// Returns the number of bytes copied in case of success.
async fn copy_to_quinn(
    mut from: impl AsyncRead + Unpin,
    mut send: quinn::SendStream,
    token: CancellationToken,
) -> io::Result<u64> {
    tracing::trace!("copying to quinn");
    tokio::select! {
        res = tokio::io::copy(&mut from, &mut send) => {
            let size = res?;
            send.finish()?;
            Ok(size)
        }
        _ = token.cancelled() => {
            // send a reset to the other side immediately
            send.reset(0u8.into()).ok();
            Err(io::Error::other("cancelled"))
        }
    }
}

/// Copy from a quinn stream to a writer.
///
/// Will send stop to the other side if the operation is cancelled, and fail
/// with an error.
///
/// Returns the number of bytes copied in case of success.
async fn copy_from_quinn(
    mut recv: quinn::RecvStream,
    mut to: impl AsyncWrite + Unpin,
    token: CancellationToken,
) -> io::Result<u64> {
    tokio::select! {
        res = tokio::io::copy(&mut recv, &mut to) => {
            Ok(res?)
        },
        _ = token.cancelled() => {
            recv.stop(0u8.into()).ok();
            Err(io::Error::other("cancelled"))
        }
    }
}

/// Get the secret key or generate a new one.
///
/// Print the secret key to stderr if it was generated, so the user can save it.
fn get_or_create_secret() -> Result<SecretKey> {
    match std::env::var("IROH_SECRET") {
        Ok(secret) => SecretKey::from_str(&secret).context("invalid secret"),
        Err(_) => {
            let key = SecretKey::generate(rand::rngs::OsRng);
            eprintln!(
                "using secret key {}",
                data_encoding::HEXLOWER.encode(&key.to_bytes())
            );
            Ok(key)
        }
    }
}

/// Create a new iroh endpoint.
async fn create_endpoint(
    secret_key: SecretKey,
    common: &CommonArgs,
    alpns: Vec<Vec<u8>>,
) -> Result<Endpoint> {
    let mut builder = Endpoint::builder().secret_key(secret_key).alpns(alpns);
    if let Some(addr) = common.ipv4_addr {
        builder = builder.bind_addr_v4(addr);
    }
    if let Some(addr) = common.ipv6_addr {
        builder = builder.bind_addr_v6(addr);
    }
    let endpoint = builder.bind().await?;
    Ok(endpoint)
}

fn cancel_token<T>(token: CancellationToken) -> impl Fn(T) -> T {
    move |x| {
        token.cancel();
        x
    }
}

/// Bidirectionally forward data from a quinn stream and an arbitrary tokio
/// reader/writer pair, aborting both sides when either one forwarder is done,
/// or when control-c is pressed.
async fn forward_bidi(
    from1: impl AsyncRead + Send + Sync + Unpin + 'static,
    to1: impl AsyncWrite + Send + Sync + Unpin + 'static,
    from2: quinn::RecvStream,
    to2: quinn::SendStream,
) -> Result<()> {
    let token1 = CancellationToken::new();
    let token2 = token1.clone();
    let token3 = token1.clone();
    let forward_from_stdin = tokio::spawn(async move {
        copy_to_quinn(from1, to2, token1.clone())
            .await
            .map_err(cancel_token(token1))
    });
    let forward_to_stdout = tokio::spawn(async move {
        copy_from_quinn(from2, to1, token2.clone())
            .await
            .map_err(cancel_token(token2))
    });
    let _control_c = tokio::spawn(async move {
        tokio::signal::ctrl_c().await?;
        token3.cancel();
        io::Result::Ok(())
    });
    forward_to_stdout.await.e()?.e()?;
    forward_from_stdin.await.e()?.e()?;
    Ok(())
}

/// Listen on a tcp port and forward incoming connections to an endpoint.
async fn connect_tcp(endpoint: &Endpoint, args: ConnectTcpArgs) -> Result<()> {
    let addrs = args
        .addr
        .to_socket_addrs()
        .context(format!("invalid host string {}", args.addr))?;
    tracing::info!("tcp listening on {:?}", addrs);

    let tcp_listener = match tokio::net::TcpListener::bind(addrs.as_slice()).await {
        Ok(tcp_listener) => tcp_listener,
        Err(cause) => {
            tracing::error!("error binding tcp socket to {:?}: {}", addrs, cause);
            return Ok(());
        }
    };
    async fn handle_tcp_accept(
        next: io::Result<(tokio::net::TcpStream, SocketAddr)>,
        addr: NodeAddr,
        endpoint: Endpoint,
        handshake: bool,
        alpn: &[u8],
    ) -> Result<()> {
        let (tcp_stream, tcp_addr) = next.context("error accepting tcp connection")?;
        let (tcp_recv, tcp_send) = tcp_stream.into_split();
        tracing::info!("got tcp connection from {}", tcp_addr);
        let remote_node_id = addr.node_id;
        let connection = endpoint
            .connect(addr, alpn)
            .await
            .context(format!("error connecting to {remote_node_id}"))?;
        let (mut endpoint_send, endpoint_recv) = connection
            .open_bi()
            .await
            .context(format!("error opening bidi stream to {remote_node_id}"))?;
        // send the handshake unless we are using a custom alpn
        // when using a custom alpn, evertyhing is up to the user
        if handshake {
            // the connecting side must write first. we don't know if there will be something
            // on stdin, so just write a handshake.
            endpoint_send.write_all(&HANDSHAKE).await.e()?;
        }
        forward_bidi(tcp_recv, tcp_send, endpoint_recv, endpoint_send).await?;
        Ok::<_, n0_snafu::Error>(())
    }
    let addr = args.ticket.node_addr();
    loop {
        // also wait for ctrl-c here so we can use it before accepting a connection
        let next = tokio::select! {
            stream = tcp_listener.accept() => stream,
            _ = tokio::signal::ctrl_c() => {
                eprintln!("got ctrl-c, exiting");
                break;
            }
        };
        let endpoint = endpoint.clone();
        let addr = addr.clone();
        let handshake = true;
        let alpn = ALPN;
        tokio::spawn(async move {
            if let Err(cause) = handle_tcp_accept(next, addr, endpoint, handshake, &alpn).await {
                // log error at warn level
                //
                // we should know about it, but it's not fatal
                tracing::warn!("error handling connection: {}", cause);
            }
        });
    }
    Ok(())
}

/// Listen on an endpoint and forward incoming connections to a tcp socket.
async fn listen_tcp(endpoint: &Endpoint, args: ListenTcpArgs) -> Result<()> {
    let addrs = match args.host.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => snafu::whatever!("invalid host string {}: {}", args.host, e),
    };

    let node_addr = endpoint.node_addr().initialized().await;
    let mut short = node_addr.clone();
    let ticket = NodeTicket::new(node_addr);
    short.direct_addresses.clear();
    let short = NodeTicket::new(short);

    // print the ticket on stderr so it doesn't interfere with the data itself
    //
    // note that the tests rely on the ticket being the last thing printed
    eprintln!("Forwarding incoming requests to '{}'.", args.host);
    eprintln!("To connect, use e.g.:");
    eprintln!("dumbpipe connect-tcp {ticket}");
    if args.common.verbose > 0 {
        eprintln!("or:\ndumbpipe connect-tcp {short}");
    }
    tracing::info!("node id is {}", ticket.node_addr().node_id);
    tracing::info!("derp url is {:?}", ticket.node_addr().relay_url);

    // handle a new incoming connection on the endpoint
    async fn handle_endpoint_accept(
        connecting: Connecting,
        addrs: Vec<std::net::SocketAddr>,
        handshake: bool,
    ) -> Result<()> {
        let connection = connecting.await.context("error accepting connection")?;
        let remote_node_id = &connection.remote_node_id()?;
        tracing::info!("got connection from {}", remote_node_id);
        let (s, mut r) = connection
            .accept_bi()
            .await
            .context("error accepting stream")?;
        tracing::info!("accepted bidi stream from {}", remote_node_id);
        if handshake {
            // read the handshake and verify it
            let mut buf = [0u8; HANDSHAKE.len()];
            r.read_exact(&mut buf).await.e()?;
            snafu::ensure_whatever!(buf == HANDSHAKE, "invalid handshake");
        }
        let connection = tokio::net::TcpStream::connect(addrs.as_slice())
            .await
            .context(format!("error connecting to {addrs:?}"))?;
        let (read, write) = connection.into_split();
        forward_bidi(read, write, r, s).await?;
        Ok(())
    }

    loop {
        let incoming = select! {
            incoming = endpoint.accept() => incoming,
            _ = tokio::signal::ctrl_c() => {
                eprintln!("got ctrl-c, exiting");
                break;
            }
        };
        let Some(incoming) = incoming else {
            break;
        };
        let Ok(connecting) = incoming.accept() else {
            break;
        };
        let addrs = addrs.clone();
        let handshake = true;
        tokio::spawn(async move {
            if let Err(cause) = handle_endpoint_accept(connecting, addrs, handshake).await {
                // log error at warn level
                //
                // we should know about it, but it's not fatal
                tracing::warn!("error handling connection: {}", cause);
            }
        });
    }
    Ok(())
}
