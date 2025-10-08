use crate::{state::AppState, Route};
use dioxus::prelude::*;

const NAVBAR_CSS: Asset = asset!("/assets/styling/navbar.css");

/// The Navbar component that will be rendered on all pages of our app since every page is under the layout.
///
///
/// This layout component wraps the UI of [Route::Home] and [Route::Blog] in a common navbar. The contents of the Home and Blog
/// routes will be rendered under the outlet inside this component
#[component]
pub fn Navbar() -> Element {
    let state = use_resource(move || async move { AppState::new().await.unwrap() });

    rsx! {
        document::Link { rel: "stylesheet", href: NAVBAR_CSS }

        match &*state.read() {
            Some(state) => {
                let id = state.node().endpoint_id();
                rsx! {
                    h5 { "endpoint_id: {id}" }
                }
            }
            None => rsx! {"Loading..."},
        }

        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
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
