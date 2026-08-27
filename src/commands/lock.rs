use crate::cli::LockArgs;
use crate::config::{ProjectContext, save_lock};
use crate::engine::SkopeoEngine;
use crate::image_reference::{ImageReference, parse_oci_reference, shorten_digest};
use crate::output::multi_println;
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn lock_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    reference_str: String,
    reference: ImageReference,
    is_insecure: bool,
) -> Result<(String, ImageReference)> {
    let _permit = semaphore.acquire().await.map_err(|e| anyhow::anyhow!(e))?;

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Locking {}...", reference.to_tag_string()));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine.inspect_remote(&reference, is_insecure).await {
        Ok(Some(inspected)) => {
            pb.finish_with_message(format!("{} locked", reference.to_tag_string()));
            Ok((reference_str, inspected))
        }
        Ok(None) => {
            multi_println(&multi, format!("Error: Image not found: {}", reference_str));
            pb.abandon_with_message(format!("{} not found", reference.to_tag_string()));
            Err(anyhow::anyhow!("Image not found: {}", reference_str))
        }
        Err(e) => {
            multi_println(&multi, format!("Error locking {}:\n{}", reference_str, e));
            pb.abandon_with_message(format!("{} failed", reference.to_tag_string()));
            Err(e)
        }
    }
}

pub async fn run_lock(
    engine: Arc<SkopeoEngine>,
    mut ctx: ProjectContext,
    args: LockArgs,
) -> Result<()> {
    let mut pruned: Vec<(String, Option<String>)> = Vec::new();
    ctx.lock.images.retain(|k, v| {
        let keep = ctx.config.dependencies.contains(k);
        if !keep {
            pruned.push((k.clone(), v.digest.clone()));
        }
        keep
    });
    let pruned_count = pruned.len();

    if pruned_count > 0 {
        for (name, digest) in &pruned {
            println!(
                "- {} removed (was locked at {})",
                name,
                digest.as_deref().map(shorten_digest).unwrap_or("-")
            );
        }
        println!("Pruned {} stale dependencies from lockfile.", pruned_count);
    }

    if ctx.config.dependencies.is_empty() {
        println!("No dependencies defined in dull.toml.");
        if pruned_count > 0 {
            save_lock(&ctx)?;
            println!("Lock file updated.");
        }
        return Ok(());
    }
    let mut parsed_deps = BTreeMap::new();
    for dep_str in &ctx.config.dependencies {
        let reference = parse_oci_reference(dep_str)
            .with_context(|| format!("Failed to parse dependency: {}", dep_str))?;

        if reference.registry.is_none() {
            bail!(
                "Short-name '{}' is forbidden for reproducibility.\n\
                 Please specify a Fully Qualified Image Name (FQIN) (e.g., 'docker.io/library/ubuntu:latest' or 'quay.io/fedora/fedora:latest').",
                dep_str
            );
        }

        if reference.tag.is_none() && reference.digest.is_none() {
            bail!(
                "Tag or digest is missing for '{}'.\n\
                 For reproducibility, you must explicitly specify a tag or digest (e.g., ':latest' or '@sha256:...').",
                dep_str
            );
        }

        parsed_deps.insert(dep_str.clone(), reference);
    }

    let mut to_resolve = Vec::new();
    let mut old_digests: BTreeMap<String, Option<String>> = BTreeMap::new();

    let target_keys: Vec<&String> = if args.images.is_empty() {
        parsed_deps.keys().collect()
    } else {
        args.images.iter().collect()
    };

    for target in target_keys {
        let reference = match parsed_deps.get(target) {
            Some(r) => r.clone(),
            None => bail!("Dependency '{}' is not defined in dull.toml.", target),
        };

        let is_locked = ctx.lock.images.contains_key(target);

        if args.upgrade {
            let old_digest = ctx.lock.images.remove(target).and_then(|r| r.digest);
            old_digests.insert(target.clone(), old_digest);
            to_resolve.push((target.clone(), reference));
        } else {
            if is_locked {
                if !args.images.is_empty() {
                    println!(
                        "Dependency '{}' is already locked.\n\
                         To overwrite it with the latest digest, use 'dull lock --upgrade {}'",
                        target, target
                    );
                }
            } else {
                old_digests.insert(target.clone(), None);
                to_resolve.push((target.clone(), reference));
            }
        }
    }

    if to_resolve.is_empty() {
        if pruned_count > 0 {
            save_lock(&ctx)?;
            println!("Lock file updated.");
            return Ok(());
        }
        if args.images.is_empty() {
            println!(
                "All dependencies are already locked.\n\
                 Use 'dull lock --upgrade' to update all, or 'dull lock --upgrade <IMAGE>' to update a specific one."
            );
        }
        return Ok(());
    }

    println!("Locking {} dependencies...", to_resolve.len());

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());

    let mut handles = Vec::with_capacity(to_resolve.len());

    for (reference_str, reference) in to_resolve {
        let semaphore_clone = Arc::clone(&semaphore);
        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);
        let is_insecure = args.insecure;

        handles.push(tokio::spawn(async move {
            lock_single_image(
                engine_clone,
                multi_clone,
                semaphore_clone,
                reference_str,
                reference,
                is_insecure,
            )
            .await
        }));
    }

    let mut failed = false;
    let mut changes = Vec::new();

    for handle in handles {
        match handle.await {
            Ok(Ok((ref_str, locked_ref))) => {
                let old_digest = old_digests.get(&ref_str).cloned().flatten();
                let new_digest = locked_ref.digest.clone();

                match old_digest {
                    None => changes.push(format!(
                        "+ {} locked at {}",
                        ref_str,
                        new_digest.as_deref().map(shorten_digest).unwrap_or("-")
                    )),
                    Some(old) if Some(old.as_str()) != new_digest.as_deref() => {
                        changes.push(format!(
                            "~ {} {} -> {}",
                            ref_str,
                            shorten_digest(&old),
                            new_digest.as_deref().map(shorten_digest).unwrap_or("-")
                        ));
                    }
                    _ => {}
                }

                ctx.lock.images.insert(ref_str, locked_ref);
            }
            Ok(Err(_)) => failed = true, // skopeo failed
            Err(_) => failed = true,     // task panicked/cancelled
        }
    }

    drop(multi);

    if failed {
        bail!("Locking completed with errors.");
    }

    save_lock(&ctx)?;

    if changes.is_empty() {
        println!("No digest changes.");
    } else {
        for change in changes {
            println!("{}", change);
        }
    }

    println!("Lock file updated.");

    Ok(())
}
