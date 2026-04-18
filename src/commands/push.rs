use crate::cli::PushArgs;
use crate::config::{ProjectContext, save_lock};
use crate::engine::ContainerEngine;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::sync::Mutex;

pub fn run_push(
    engine: &impl ContainerEngine,
    mut ctx: ProjectContext,
    args: PushArgs,
) -> Result<()> {
    let project = ctx
        .config
        .project
        .as_ref()
        .context("Project configuration missing")?;
    let registry_base = project
        .registries
        .get("default")
        .context("Default registry not configured")?;

    // push対象の絞り込み (all or specific images)
    let target_names: Vec<String> = if args.all {
        ctx.lock.images.keys().cloned().collect()
    } else {
        args.images.clone()
    };

    println!(
        "Pushing {} images to {}...",
        target_names.len(),
        registry_base
    );

    // 並列実行時のロック更新用（ビルドの時と同じパターン）
    let update_buffer = Mutex::new(Vec::new());

    // ネットワークI/Oの塊なので、rayonで並列化して限界まで帯域を使い切る
    target_names.par_iter().for_each(|name| {
        let lock_info = match ctx.lock.images.get(name) {
            Some(info) => info,
            None => {
                eprintln!("Warning: No lock entry for {}. Build it first.", name);
                return;
            }
        };

        // ローカルのタグ情報をdull.tomlから引く
        let local_tag = ctx
            .config
            .images
            .get(name)
            .map(|p| p.local_tag.as_str())
            .unwrap_or("latest");

        // 例: docker://registry.example.com/my-project/app1:latest
        let destination = format!("docker://{}/{}:{}", registry_base, name, local_tag);

        println!("-> Pushing {}...", name);
        match engine.push(&lock_info.local_ref, &destination) {
            Ok(registry_digest) => {
                println!("✓ {} pushed. Digest: {}", name, registry_digest);
                // 成功したらバッファに (名前, リモートダイジェスト) を積む
                update_buffer
                    .lock()
                    .unwrap()
                    .push((name.clone(), registry_digest));
            }
            Err(e) => {
                eprintln!("✗ Failed to push {}: {}", name, e);
                // エラー時のFail-safe/Fail-fast制御は今後の課題だが、今はエラーを出して続行する
            }
        }
    });
    let updates = update_buffer.into_inner().unwrap();
    let mut updated_count = 0;
    for (name, digest) in updates {
        if let Some(info) = ctx.lock.images.get_mut(&name) {
            // "default" レジストリにプッシュした結果として保存
            info.registry_digests.insert("default".to_string(), digest);
            updated_count += 1;
        }
    }

    // 更新があった場合のみ、アトミックにdull.lockを保存
    if updated_count > 0 {
        save_lock(&ctx)?;
        println!(
            "🔒 dull.lock updated with {} registry digests.",
            updated_count
        );
    } else {
        println!("No images were successfully pushed.");
    }

    Ok(())
}
