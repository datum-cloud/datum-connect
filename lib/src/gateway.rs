use std::{io, net::SocketAddr};

use askama::Template;
use hyper::StatusCode;
use iroh_proxy_utils::{
    HttpRequest, HttpResponse,
    downstream::{
        EndpointAuthority, ExtractEndpointAuthority, ExtractError, ProxyOpts, WriteErrorResponse,
    },
};
use n0_error::Result;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    net::TcpListener,
};
use tracing::{debug, info};

use crate::{TicketClient, node::ConnectNode};

pub async fn serve(node: ConnectNode, bind_addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    info!(?bind_addr, endpoint_id = %node.endpoint_id().fmt_short(),"TCP proxy gateway started");

    let extractor = Resolver {
        tickets: node.tickets,
    };
    let opts = ProxyOpts::reverse_only(extractor).with_error_response_writer(ErrorResponseWriter);

    node.proxy.forward_tcp_listener(listener, opts).await
}

#[derive(Clone)]
struct Resolver {
    tickets: TicketClient,
}

impl ExtractEndpointAuthority for Resolver {
    async fn extract_endpoint_authority<'a>(
        &'a self,
        req: &'a HttpRequest,
    ) -> Result<EndpointAuthority, ExtractError> {
        let host = req.headers.get("host").ok_or(ExtractError::BadRequest)?;
        let host = host.to_str().map_err(|_| ExtractError::BadRequest)?;
        let codename = extract_subdomain(host).ok_or(ExtractError::NotFound)?;

        debug!(%codename, "extracted codename, fetching ticket...");
        let ticket = self.tickets.get(codename).await.map_err(|err| {
            debug!(%codename, "failed to fetch ticket: {err:#}");
            ExtractError::NotFound
        })?;
        debug!(?ticket, "fetched ticket");
        Ok(EndpointAuthority {
            endpoint_id: ticket.endpoint,
            authority: ticket.data.data.into(),
        })
    }
}

pub(super) fn extract_subdomain(host: &str) -> Option<&str> {
    let host = host
        .rsplit_once(':')
        .map(|(host, _port)| host)
        .unwrap_or(host);
    if host.parse::<std::net::IpAddr>().is_ok() {
        None
    } else {
        host.split_once(".").map(|(first, _rest)| first)
    }
}

#[derive(Template)]
#[template(path = "gateway_error.html")]
struct GatewayErrorTemplate<'a> {
    title: &'a str,
    body: &'a str,
}

struct ErrorResponseWriter;

impl WriteErrorResponse for ErrorResponseWriter {
    async fn write_error_response<'a>(
        &'a self,
        res: &'a HttpResponse,
        writer: &'a mut (dyn AsyncWrite + Send + Unpin),
    ) -> io::Result<()> {
        let title = format!("{} {}", res.status.as_u16(), res.reason());
        let body = match res.status {
            StatusCode::NOT_FOUND => {
                "The requested proxy was not found. Please check the domain and try again."
            }
            StatusCode::GATEWAY_TIMEOUT => "The requested proxy is unavailable.",
            StatusCode::BAD_GATEWAY => "The requested proxy is malfunctioning.",
            StatusCode::INTERNAL_SERVER_ERROR => {
                "The gateway is experiencing problems. Please try again later."
            }
            StatusCode::BAD_REQUEST => "You performed an invalid request.",
            _ => "The service experienced an error",
        };
        let html = GatewayErrorTemplate {
            body: &body,
            title: &title,
        }
        .render()
        .unwrap_or(title);
        // let html_body = include_str!("../static/gateway_not_found.html");
        writer.write_all(res.status_line().as_bytes()).await?;
        let headers = format!(
            "Content-Type: text/html\r\nContent-Length: {}\r\n\r\n",
            html.len()
        );
        writer.write_all(headers.as_bytes()).await?;
        writer.write_all(html.as_bytes()).await?;
        Ok(())
    }
}
