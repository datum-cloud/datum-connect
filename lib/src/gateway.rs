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

use crate::{FetchTicketError, TicketClient, node::ConnectNode};

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

        debug!(%codename, "extracted codename, resolving ticket...");
        let ticket = self.tickets.get(codename).await.map_err(|err| {
            debug!(%codename, "failed to resolve ticket: {err:#}");
            match err {
                FetchTicketError::NotFound => ExtractError::NotFound,
                FetchTicketError::FailedToFetch(_) => ExtractError::ServiceUnavailable,
            }
        })?;
        debug!(?ticket, "resolved ticket");
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
            StatusCode::BAD_REQUEST => {
                "The request could not be understood by the gateway. Please try again."
            }
            StatusCode::UNAUTHORIZED => {
                "You are not logged in or your session has expired. Please sign in and try again."
            }
            StatusCode::FORBIDDEN => "Access to this resource is not allowed through the gateway.",
            StatusCode::NOT_FOUND => "The requested page could not be found through the gateway.",
            StatusCode::INTERNAL_SERVER_ERROR => {
                "The gateway encountered an internal error. Please try again later."
            }
            StatusCode::BAD_GATEWAY => {
                "The gateway could not get a valid response from the upstream service."
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                "The service is temporarily unavailable. Please try again shortly."
            }
            StatusCode::GATEWAY_TIMEOUT => "The upstream service took too long to respond.",
            _ => "The service experienced an unexpected error.",
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
