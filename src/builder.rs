use crate::BrowserTransport;

pub struct BrowserTransportBuilder {
    pub name: String,
    pub description: String,
    pub url: String,
    pub wc_project_id: Option<String>,
    pub icons: Vec<String>,
    pub rpc_node: Option<String>,
}

impl BrowserTransportBuilder {
    pub fn new() -> Self {
        Self {
            name: "Example dApp".to_string(),
            description: "An example dApp written in Rust".to_string(),
            url: "https://github.com/quay-rs/ethers-web".to_string(),
            wc_project_id: None,
            icons: Vec::new(),
            rpc_node: None,
        }
    }

    pub fn name(&mut self, name: &str) -> &Self {
        self.name = name.to_string();
        self
    }

    pub fn description(&mut self, description: &str) -> &Self {
        self.description = description.to_string();
        self
    }

    pub fn url(&mut self, url: &str) -> &Self {
        self.url = url.to_string();
        self
    }

    pub fn walletconnect_id(&mut self, wc_project_id: &str) -> &Self {
        self.wc_project_id = Some(wc_project_id.to_string());
        self
    }

    pub fn rpc_node(&mut self, rpc_node: &str) -> &Self {
        self.rpc_node = Some(rpc_node.to_string());
        self
    }

    pub fn add_icon(&mut self, icon_url: &str) -> &Self {
        self.icons.push(icon_url.to_string());
        self
    }

    pub fn build(&self) -> BrowserTransport {
        BrowserTransport::new(
//            self.name.clone(),
//            self.description.clone(),
//            self.url.clone(),
//            self.wc_project_id.clone(),
//            self.icons.clone(),
//            self.rpc_node.clone(),
        )
    }
}