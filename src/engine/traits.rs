use anyhow::Result;
use std::path::Path;

use crate::config::ImageProfile;

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
