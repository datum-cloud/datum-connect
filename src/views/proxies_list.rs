use dioxus::prelude::*;

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn TempProxies() -> Element {
    rsx! {
        h1{ "TODO: temp proxies" }
    }
}
