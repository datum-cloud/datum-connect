use dioxus::prelude::*;

use crate::{
    node::{TcpConnection, TcpListener},
    state::AppState,
    Route,
};

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn TempProxies() -> Element {
    let mut connections = use_signal(|| Vec::new());
    let mut listeners = use_signal(|| Vec::new());
    use_future(move || async move {
        let state = consume_context::<AppState>();
        let node = state.node();
        let conns = node.connections().await;
        connections.set(conns);
        let lstnrs = node.listeners().await;
        listeners.set(lstnrs);
    });

    rsx! {
        h3{ "Connections" }
        Link {
            to: Route::JoinProxy {  },
            "Join Proxy"
        }
        for conn in connections() {
            ProxyConnectionItem { conn, connections }
        }
        h3{ "Listeners" }
        Link {
            to: Route::CreateProxy {  },
            "Create Proxy"
        }
        for lstn in listeners() {
            ProxyListenerItem { lstn, listeners }
        }
    }
}

#[component]
fn ProxyConnectionItem(conn: TcpConnection, connections: Signal<Vec<TcpConnection>>) -> Element {
    rsx! {
        div {
            h2 { "Proxy Connection" }
            p { "Status: active" }
            p { "Address: {conn.addr}" }
            button {
                onclick: move |_| {
                    let conn_2 = conn.clone();
                    async move {
                        let state = consume_context::<AppState>();
                        // TODO(b5) - remove unwrap
                        let node = state.node();
                        node.disconnect_tcp(&conn_2).await.unwrap();
                        let conns = node.connections().await;
                        connections.set(conns);
                    }
                },
                "Disconnect"
            }
        }
    }
}

#[component]
fn ProxyListenerItem(lstn: TcpListener, listeners: Signal<Vec<TcpListener>>) -> Element {
    rsx! {
        div {
            h2 { "Proxy Listener" }
            p { "Status: active" }
            p { "Address: {lstn.addr}" }
            p { "Ticket: {lstn.ticket}" },
            button {
                onclick: move |_| {
                    let lstn_2 = lstn.clone();
                    async move {
                        let state = consume_context::<AppState>();
                        let node = state.node();
                        // TODO(b5) - remove unwrap
                        node.unlisten_tcp(&lstn_2).await.unwrap();
                        let lstns = node.listeners().await;
                        listeners.set(lstns);
                    }
                },
                "Disconnect"
            }
        }
    }
}
