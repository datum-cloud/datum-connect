use lib::domains::{example_domains, Domain};
use lib::{Node, Repo};

#[derive(Debug, Clone)]
pub struct AppState {
    domains: Vec<Domain>,
    node: Node,
}

impl AppState {
    pub async fn load() -> anyhow::Result<Self> {
        // TODO - fml these unwraps
        let repo = Repo::open_or_create(Repo::default_location()).await?;
        let node = repo.spawn_node().await?;

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
