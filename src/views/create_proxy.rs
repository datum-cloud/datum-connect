use dioxus::prelude::*;

use crate::state::AppState;

/// The Blog page component that will be rendered when the current route is `[Route::Blog]`
///
/// The component takes a `id` prop of type `i32` from the route enum. Whenever the id changes, the component function will be
/// re-run and the rendered HTML will be updated.
#[component]
pub fn CreateProxy() -> Element {
    let mut port = use_signal(|| "localhost:5173".to_string());
    let mut ticket = use_signal(|| "Click to generate a ticket".to_string());

    rsx! {
        div {
            id: "create-proxy",
            // h4 { "domain" },
            // input {
            //     placeholder: "Domain Name",
            //     value: "example.com",
            //     onchange: move |e| {
            //         // Handle input change event
            //     }
            // },
            h4 { "local port to forward" },
            input {
                placeholder: "Port",
                value: "{port}",
                onchange: move |e| {
                    port.set(e.value());
                }
            }
            button {
                class: "cursor-pointer",
                onclick: move |_| async move {
                    let state = consume_context::<AppState>();
                    let tkt = state.clone().node().listen_tcp(port()).await.unwrap();
                    ticket.set(tkt.to_string())
                },
                "Create"
            }
            div {
                id: "ticket-container",
                p { "{ticket}" },
            }
        }
    }
}
