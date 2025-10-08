use crate::Route;
use dioxus::prelude::*;

const PROXY_CSS: Asset = asset!("/assets/styling/proxy.css");

/// The Blog page component that will be rendered when the current route is `[Route::Blog]`
///
/// The component takes a `id` prop of type `i32` from the route enum. Whenever the id changes, the component function will be
/// re-run and the rendered HTML will be updated.
#[component]
pub fn CreateProxy() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: PROXY_CSS }

        div {
            id: "create-proxy",
            h1 { "Create Proxy" },
            button {
                class: "cursor-pointer",
                onclick: move |_| {
                    // Handle button click event
                },
                "Create"
            }
        }
    }
}
