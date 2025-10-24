use dioxus::prelude::*;
use lib::domains::Domain;

use crate::{
    components::{Button, Subhead},
    Route,
};

#[derive(Props, PartialEq, Clone)]
pub struct DomainProps {
    pub domains: Vec<Domain>,
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

#[derive(Props, PartialEq, Clone)]
struct DomainItemProps {
    domain: Domain,
}

#[component]
fn DomainItem(props: DomainItemProps) -> Element {
    let domain = &props.domain;
    rsx! {
        div {
            class: "my-5",
            Subhead { text: "{domain.name}" }
            a {
                class: "text-yellow-500",
                href: "{domain.url}",
                "{domain.url}"
            }
        }
    }
}
