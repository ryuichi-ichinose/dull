use anyhow::Result;
use std::future::Future;
use std::path::Path;

#[derive(Debug)]
pub struct BuildOptions<'a> {
    pub dockerfile_path: &'a Path,
    pub context_dir: &'a Path,
    pub target_tag: &'a str,
    pub build_args: &'a std::collections::BTreeMap<String, String>,
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
    fn login<'a>(
        &'a self,
        registry_url: &'a str,
        auth: &'a RegistryAuth<'a>,
    ) -> impl Future<Output = Result<()>> + Send + 'a;

    fn build<'a>(
        &'a self,
        name: &'a str,
        opts: BuildOptions<'a>,
    ) -> impl Future<Output = Result<BuildResult>> + Send + 'a;

    fn push<'a>(
        &'a self,
        local_ref: &'a str,
        destination: &'a str,
    ) -> impl Future<Output = Result<String>> + Send + 'a;
}
