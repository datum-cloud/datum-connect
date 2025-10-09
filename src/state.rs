use crate::node::Node;

#[derive(Debug, Clone)]
pub struct AppState {
    node: Node,
}

impl AppState {
    pub async fn load() -> anyhow::Result<Self> {
        // TODO - fml.
        let node = Node::new().await.unwrap();
        Ok(AppState { node })
    }

    pub fn node(&self) -> &Node {
        &self.node
    }
}
