use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ImageProfile {
    pub path: String,
}

pub trait ContainerEngine {
    fn login(
        &self,
        registry_url: &str,
        username: Option<&str>,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()>;
    fn build(&self, root: &Path, name: &str, profile: &ImageProfile, tag: &str) -> Result<String>;
    fn push(&self, registry: &str, name: &str, digest: &str, tag: &str, insecure: bool)
        -> Result<()>;
}
