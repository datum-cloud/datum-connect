use crate::{state::AppState, Route};
use dioxus::prelude::*;

/// The Navbar component that will be rendered on all pages of our app since every page is under the layout.
///
///
/// This layout component wraps the UI of [Route::Home] and [Route::Blog] in a common navbar. The contents of the Home and Blog
/// routes will be rendered under the outlet inside this component
#[component]
pub fn Navbar() -> Element {
    let state = consume_context::<AppState>();
    let id = state.node().endpoint_id();

    rsx! {
        h5 { class: "text-xl text-gray-800", "endpoint_id: {id}" },
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Domains"
            }
            Link {
                to: Route::CreateProxy {},
                "Create Proxy"
            }
        }

        // The `Outlet` component is used to render the next component inside the layout. In this case, it will render either
        // the [`Home`] or [`Blog`] component depending on the current route.
        Outlet::<Route> {}
    }
}
