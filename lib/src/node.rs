use anyhow::anyhow;
use iroh::{Endpoint, EndpointAddr, PublicKey, SecretKey, endpoint::Connecting};
use iroh_tickets::endpoint::EndpointTicket;
use n0_snafu::{Result, ResultExt};
use snafu::whatever;
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    select,
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;
use tracing::debug;
use uuid::Uuid;

use crate::config::Config;

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
    pub async fn new(secret_key: SecretKey, config: &Config) -> Result<Self> {
        let endpoint = create_endpoint(secret_key, config, vec![ALPN.to_vec()]).await?;

        // wait for the endpoint to figure out its address before making a ticket
        endpoint.online().await;
        let node_addr = endpoint.addr();

        Ok(Self {
            inner: Arc::new(NodeInner {
                ep_id: node_addr.id,
                endpoint,
                tcp_listeners: Mutex::new(Vec::new()),
                tcp_connections: Mutex::new(Vec::new()),
            }),
        })
    }

    pub fn endpoint_id(&self) -> String {
        self.inner.ep_id.to_string()
    }

    pub async fn listeners(&self) -> Vec<TcpListener> {
        self.inner
            .tcp_listeners
            .lock()
            .await
            .iter()
            .map(TcpListener::from)
            .collect()
    }

    pub async fn connections(&self) -> Vec<TcpConnection> {
        self.inner
            .tcp_connections
            .lock()
            .await
            .iter()
            .map(TcpConnection::from)
            .collect()
    }

    pub async fn listen_tcp(&self, label: String, host: String) -> Result<EndpointTicket> {
        let listener = listen_tcp(&self.inner.endpoint, label, host).await?;
        let ticket = listener.ticket.clone();
        let mut tcp_listeners = self.inner.tcp_listeners.lock().await;
        tcp_listeners.push(listener);
        Ok(ticket)
    }

    pub async fn unlisten_tcp(&self, lstn: &TcpListener) -> anyhow::Result<()> {
        let mut lstrs = self.inner.tcp_listeners.lock().await;
        let mut found = false;
        debug!("unlisten tcp. id: {:?}", lstn.id);
        lstrs.retain(|h| {
            if h.id == lstn.id {
                h.handle.abort();
                found = true;
                false
            } else {
                true
            }
        });
        match found {
            true => Ok(()),
            false => Err(anyhow!("TCP listener not found")),
        }
    }

    pub async fn connect_tcp(
        &self,
        label: String,
        addr: String,
        ticket: EndpointTicket,
    ) -> Result<()> {
        let conn = connect_tcp(&self.inner.endpoint, label, addr, ticket).await?;
        let mut tcp_connections = self.inner.tcp_connections.lock().await;
        tcp_connections.push(conn);
        Ok(())
    }

    pub async fn disconnect_tcp(&self, conn: &TcpConnection) -> anyhow::Result<()> {
        let mut conns = self.inner.tcp_connections.lock().await;
        let mut found = false;
        debug!("disconnect tcp. id: {:?}", conn.id);
        conns.retain(|h| {
            if h.id == conn.id {
                h.handle.abort();
                found = true;
                false
            } else {
                true
            }
        });
        match found {
            true => Ok(()),
            false => Err(anyhow!("TCP connection not found")),
        }
    }
}

#[derive(Debug)]
struct NodeInner {
    ep_id: PublicKey,
    endpoint: Endpoint,
    tcp_listeners: Mutex<Vec<TcpListenerHandle>>,
    tcp_connections: Mutex<Vec<TcpConnectionHandle>>,
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

/// Create a new iroh endpoint.
async fn create_endpoint(
    secret_key: SecretKey,
    common: &Config,
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

#[derive(Debug, Clone, PartialEq)]
pub struct TcpConnection {
    pub id: Uuid,
    pub label: String,
    pub addr: String,
    pub ticket: EndpointTicket,
}

impl From<&TcpConnectionHandle> for TcpConnection {
    fn from(handle: &TcpConnectionHandle) -> Self {
        TcpConnection {
            id: handle.id.clone(),
            label: handle.label.clone(),
            addr: handle.addr.clone(),
            ticket: handle.ticket.clone(),
        }
    }
}

#[derive(Debug)]
pub struct TcpConnectionHandle {
    id: Uuid,
    label: String,
    addr: String,
    ticket: EndpointTicket,
    handle: tokio::task::JoinHandle<()>,
}

/// Listen on a tcp port and forward incoming connections to an endpoint.
/// The addresses to listen on for incoming tcp connections.
///
/// To listen on all network interfaces, use 0.0.0.0:12345
/// The node to connect to
async fn connect_tcp(
    endpoint: &Endpoint,
    label: String,
    addr: String,
    ticket: EndpointTicket,
) -> Result<TcpConnectionHandle> {
    let addr_string = addr.clone();
    let addrs = addr
        .to_socket_addrs()
        .context(format!("invalid host string {}", addr))?;
    tracing::info!("tcp listening on {:?}", addrs);

    let tcp_listener = match tokio::net::TcpListener::bind(addrs.as_slice()).await {
        Ok(tcp_listener) => tcp_listener,
        Err(cause) => {
            tracing::error!("error binding tcp socket to {:?}: {}", addrs, cause);
            whatever!("error binding tcp socket to {:?}: {}", addrs, cause);
        }
    };
    async fn handle_tcp_accept(
        next: io::Result<(tokio::net::TcpStream, SocketAddr)>,
        addr: EndpointAddr,
        endpoint: Endpoint,
        handshake: bool,
        alpn: &[u8],
    ) -> Result<()> {
        let (tcp_stream, tcp_addr) = next.context("error accepting tcp connection")?;
        let (tcp_recv, tcp_send) = tcp_stream.into_split();
        tracing::info!("got tcp connection from {}", tcp_addr);
        let remote_ep_id = addr.id;
        let connection = endpoint
            .connect(addr, alpn)
            .await
            .context(format!("error connecting to {remote_ep_id}"))?;
        let (mut endpoint_send, endpoint_recv) = connection
            .open_bi()
            .await
            .context(format!("error opening bidi stream to {remote_ep_id}"))?;
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
    let ticket2 = ticket.clone();
    let addr = ticket2.endpoint_addr().clone();
    let endpoint = endpoint.clone();
    let handle = tokio::spawn(async move {
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
                if let Err(cause) = handle_tcp_accept(next, addr, endpoint, handshake, &alpn).await
                {
                    // log error at warn level
                    //
                    // we should know about it, but it's not fatal
                    tracing::warn!("error handling connection: {}", cause);
                }
            });
        }
    });
    Ok(TcpConnectionHandle {
        id: Uuid::new_v4(),
        label,
        addr: addr_string,
        ticket,
        handle,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct TcpListener {
    pub id: Uuid,
    pub label: String,
    pub addr: String,
    pub ticket: EndpointTicket,
}

impl From<&TcpListenerHandle> for TcpListener {
    fn from(handle: &TcpListenerHandle) -> Self {
        TcpListener {
            id: handle.id.clone(),
            label: handle.label.clone(),
            addr: handle.addr.clone(),
            ticket: handle.ticket.clone(),
        }
    }
}

#[derive(Debug)]
pub struct TcpListenerHandle {
    id: Uuid,
    pub label: String,
    pub addr: String,
    pub ticket: EndpointTicket,
    pub handle: tokio::task::JoinHandle<()>,
}

/// Listen on an endpoint and forward incoming connections to a tcp socket.
async fn listen_tcp(endpoint: &Endpoint, label: String, host: String) -> Result<TcpListenerHandle> {
    let addrs = match host.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => snafu::whatever!("invalid host string {}: {}", host, e),
    };

    endpoint.online().await;
    let endpoint_addr = endpoint.addr();
    let mut short = endpoint_addr.clone();
    short.addrs.clear();
    let ticket = EndpointTicket::new(short);

    // handle a new incoming connection on the endpoint
    async fn handle_endpoint_accept(
        connecting: Connecting,
        addrs: Vec<std::net::SocketAddr>,
        handshake: bool,
    ) -> Result<()> {
        let connection = connecting.await.context("error accepting connection")?;
        let remote_node_id = &connection.remote_id()?;
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

    let endpoint = endpoint.clone();
    let handle = tokio::spawn(async move {
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
    });

    Ok(TcpListenerHandle {
        id: Uuid::new_v4(),
        label,
        addr: host,
        ticket,
        handle,
    })
}
