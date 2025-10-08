use dioxus::prelude::*;

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
            h1 { "Domains" },
            for domain in domains {
                div {
                    h2 { "{domain.name}" }
                    a { href: "{domain.url}", "{domain.url}" }
                }
            }
        }
    }
}
