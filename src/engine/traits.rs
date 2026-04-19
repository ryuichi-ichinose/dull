use anyhow::Result;
use std::path::Path;

#[derive(Debug)]
pub struct BuildOptions<'a> {
    pub dockerfile_path: &'a Path,
    pub context_dir: &'a Path,
    pub target_tag: &'a str,
    pub progress_bar: Option<indicatif::ProgressBar>,
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub digest: String,
    pub local_ref: String,
}

#[derive(Debug)]
pub struct RegistryAuth<'a> {
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
    pub extra_args: &'a [String],
}

pub trait ContainerEngine: Sync + Send {
    fn login(&self, registry_url: &str, auth: &RegistryAuth) -> Result<()>;
    fn build(&self, name: &str, opts: BuildOptions) -> Result<BuildResult>;
    fn push(&self, local_ref: &str, destination: &str) -> anyhow::Result<String>;
}
