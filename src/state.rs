use crate::{
    domains::{example_domains, Domain},
    node::Node,
};

#[derive(Debug, Clone)]
pub struct AppState {
    domains: Vec<Domain>,
    node: Node,
}

impl AppState {
    pub async fn load() -> anyhow::Result<Self> {
        // TODO - fml.
        let node = Node::new().await.unwrap();

        Ok(AppState {
            node,
            domains: example_domains(),
        })
    }

    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn domains(&self) -> Vec<Domain> {
        self.domains.clone()
    }
}
