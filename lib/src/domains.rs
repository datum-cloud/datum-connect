#[derive(PartialEq, Debug, Clone)]
pub struct Domain {
    pub name: String,
    pub url: String,
}

pub fn example_domains() -> Vec<Domain> {
    vec![
        Domain {
            name: "dev server".to_string(),
            url: "https://devserver.b5.proxy.datum.net".to_string(),
        },
        Domain {
            name: "homeserver".to_string(),
            url: "https://homeserver.b5.proxy.datum.net".to_string(),
        },
        Domain {
            name: "localllm".to_string(),
            url: "https://localllm.b5.proxy.datum.net".to_string(),
        },
    ]
}
