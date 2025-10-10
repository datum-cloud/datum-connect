use dioxus::prelude::*;

use crate::{
    components::{Button, Subhead},
    Route,
};

#[derive(Props, PartialEq, Clone)]
pub struct DomainProps {
    pub domains: Vec<Domain>,
}

#[derive(PartialEq, Clone)]
pub struct Domain {
    pub name: String,
    pub url: String,
}

#[component]
pub fn Domains(domains: Vec<Domain>) -> Element {
    rsx! {
        div {
            id: "domains",
            div {
                class: "flex",
                Subhead { text: "Domains" },
                div { class: "flex-grow" },
                Button{ to: Route::CreateProxy {  }, text: "Create Proxy" },
            }
            div {
                class: "flex flex-col space-y-4",
                for domain in domains {
                    DomainItem { domain }
                }
            }
        }
    }
}

#[component]
fn DomainItem(domain: Domain) -> Element {
    rsx! {
        div {
            h2 { "{domain.name}" }
            a { class: "text-green-500", href: "{domain.url}", "{domain.url}" }
        }
    }
}
