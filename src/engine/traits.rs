use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_tag() -> String {
    "latest".to_string()
}

fn default_context() -> String {
    ".".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ImageProfile {
    pub path: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_tag")]
    pub local_tag: String,
    #[serde(default = "default_context")]
    pub context: String,
}

pub trait ContainerEngine: Sync + Send {
    fn login(
        &self,
        registry_url: &str,
        username: Option<&str>,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()>;
    fn build(&self, root: &Path, name: &str, profile: &ImageProfile, tag: &str) -> Result<(String, String)>;
    fn push(&self, registry: &str, name: &str, digest: &str, tag: &str, insecure: bool)
        -> Result<()>;
}
