use dioxus::prelude::*;

use crate::Route;

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn TempProxies() -> Element {
    rsx! {
        h1{ "TODO: temp proxies" }
        Link {
            to: Route::CreateProxy {  },
            "Create Proxy"
        }
        Link {
            to: Route::JoinProxy {  },
            "Join Proxy"
        }
    }
}
