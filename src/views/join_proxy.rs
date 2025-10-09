use std::str::FromStr;

use dioxus::prelude::*;
use iroh_base::ticket::NodeTicket;

use crate::state::AppState;

/// The Blog page component that will be rendered when the current route is `[Route::Blog]`
///
/// The component takes a `id` prop of type `i32` from the route enum. Whenever the id changes, the component function will be
/// re-run and the rendered HTML will be updated.
#[component]
pub fn JoinProxy() -> Element {
    rsx! {
        div {
            id: "create-domain",
            h1 { "TODO: join proxy" }
            button {
                class: "cursor-pointer",
                onclick: move |_| async move {
                    let state = consume_context::<AppState>();
                    let ticket = NodeTicket::from_str("example_ticket").unwrap();
                    state.clone().node().connect_tcp("localhost:5173".to_string(), ticket).await.unwrap();
                },
                "Join"
            }

        }
    }
}
