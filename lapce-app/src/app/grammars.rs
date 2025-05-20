use std::{
    env,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use log::trace;

use crate::update::{ReleaseAsset, ReleaseInfo};

#[allow(dead_code)]
async fn get_github_api(url: &str) -> Result<String> {
    let user_agent = format!("Lapce/{}", lapce_core::meta::VERSION);
    let resp = lapce_proxy::async_get_url(url, Some(user_agent.as_str())).await?;
    if !resp.status().is_success() {
        return Err(anyhow!("get release info failed {}", resp.text().await?));
    }

    Ok(resp.text().await?)
}
#[allow(dead_code)]
pub async fn find_grammar_release() -> Result<ReleaseInfo> {
    let releases: Vec<ReleaseInfo> = serde_json::from_str(&get_github_api(
        "https://api.github.com/repos/lapce/tree-sitter-grammars/releases?per_page=100",
    ).await.context("Failed to retrieve releases for tree-sitter-grammars")?)?;

    // use lapce_core::meta::{RELEASE, ReleaseType, VERSION};

    // let releases = releases
    //     .into_iter()
    //     .filter_map(|f| {
    //         if matches!(RELEASE, ReleaseType::Debug | ReleaseType::Nightly) {
    //             return Some(f);
    //         }

    //         let tag_name = if f.tag_name.starts_with('v') {
    //             f.tag_name.trim_start_matches('v')
    //         } else {
    //             f.tag_name.as_str()
    //         };

    //         use std::cmp::Ordering;

    //         use semver::Version;

    //         let sv = Version::parse(tag_name).ok()?;
    //         let version = Version::parse(VERSION).ok()?;

    //         if matches!(sv.cmp_precedence(&version), Ordering::Equal) {
    //             Some(f)
    //         } else {
    //             None
    //         }
    //     })
    //     .collect::<Vec<_>>();

    let Some(release) = releases.first() else {
        return Err(anyhow!("Couldn't find any release"));
    };
    Ok(release.to_owned())
}
pub async fn fetch_grammars(
    release: &ReleaseInfo,
    grammars_directory: &Path,
) -> Result<bool> {
    // let dir = Directory::grammars_directory().await
    //     .ok_or_else(|| anyhow!("can't get grammars directory"))?;

    let file_name = format!("grammars-{}-{}", env::consts::OS, env::consts::ARCH);

    let updated = download_release(grammars_directory, release, &file_name).await?;

    trace!("Successfully downloaded grammars");

    Ok(updated)
}
pub async fn fetch_queries(
    release: &ReleaseInfo,
    queries_directory: &Path,
) -> Result<bool> {
    // let dir = Directory::queries_directory()
    //     .ok_or_else(|| anyhow!("can't get queries directory"))?;

    let file_name = "queries";

    let updated = download_release(queries_directory, release, file_name).await?;

    trace!("Successfully downloaded queries");

    Ok(updated)
}
async fn download_release(
    dir: &Path,
    release: &ReleaseInfo,
    file_name: &str,
) -> Result<bool> {
    use tokio::fs;
    if !dir.exists() {
        fs::create_dir(&dir).await?;
    }

    let current_version = fs::read_to_string(dir.join("version"))
        .await
        .unwrap_or_default();
    let release_version = if release.tag_name == "nightly" {
        format!("nightly-{}", &release.target_commitish[..7])
    } else {
        release.tag_name.clone()
    };

    if release_version == current_version {
        return Ok(false);
    }

    for asset in &release.assets {
        download_release_asset(dir, file_name, asset, &release_version)
            .await
            .map_err(|x| anyhow!("download {:?} fail: {}", asset, x))?;
    }
    Ok(true)
}

async fn download_release_asset(
    dir: &Path,
    file_name: &str,
    asset: &ReleaseAsset,
    release_version: &str,
) -> Result<bool> {
    use tokio::fs;

    if asset.name.starts_with(file_name) {
        let resp = lapce_proxy::async_get_url(&asset.browser_download_url, None)
            .await
            .map_err(|x| {
                anyhow!(
                    "browser_download_url {} fail: {}",
                    asset.browser_download_url,
                    x
                )
            })?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "download {} error {}",
                asset.browser_download_url,
                resp.text().await?
            ));
        }

        let mut file = tempfile::tempfile()?;

        {
            let bytes = resp.bytes().await?;
            file.write_all(&bytes)?;
            file.flush()?;
        }
        file.seek(SeekFrom::Start(0))?;

        if asset.name.ends_with(".zip") {
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(dir)?;
        } else if asset.name.ends_with(".tar.zst") {
            let mut archive =
                tar::Archive::new(zstd::stream::read::Decoder::new(file)?);
            archive.unpack(dir)?;
        }

        fs::write(dir.join("version"), &release_version).await?;
    }
    Ok(true)
}
