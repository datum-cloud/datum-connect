use dioxus::prelude::*;

use crate::{
    components::{Button, Domains},
    state::AppState,
    Route,
};

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn DomainsList() -> Element {
    let state = consume_context::<AppState>();

    rsx! {
        Domains { domains: state.domains() },
        div {
            class: "flex gap-10",
            Button {
                to: Route::Login {  },
                text: "Login"
            }
            Button {
                to: Route::Signup {  },
                text: "Signup"
            }
        }
    }
}
