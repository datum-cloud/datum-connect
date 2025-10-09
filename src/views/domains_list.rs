use dioxus::prelude::*;

use crate::{
    components::{Domain, Domains},
    Route,
};

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn DomainsList() -> Element {
    let domains = vec![
        Domain {
            name: "example.com".to_string(),
            url: "https://example.com".to_string(),
        },
        Domain {
            name: "example.org".to_string(),
            url: "https://example.org".to_string(),
        },
        Domain {
            name: "example.net".to_string(),
            url: "https://example.net".to_string(),
        },
    ];

    rsx! {
        Domains { domains },
        Link {
            to: Route::Login {  },
            "Login"
        }
        Link {
            to: Route::Signup {  },
            "Signup"
        }
    }
}
