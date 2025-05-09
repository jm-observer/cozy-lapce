use std::path::Path;

use cargo::core::{Package, Target};

/// 从 `src/` 下的模块中推导模块路径
pub fn from_src_path(package: &Package, file_path: &Path) -> Option<String> {
    let crate_name = package.name().replace('-', "_");
    let src_root = package.root().join("src");
    let rel_path = file_path.strip_prefix(&src_root).ok()?;

    let mut components = rel_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let last = components.pop();
    let mut parts = vec![crate_name];
    parts.extend(components);
    if let Some(last) = last {
        if last.ends_with(".rs") && last != "mod.rs" {
            parts.push(last.trim_end_matches(".rs").to_string());
        }
    }
    Some(parts.join("::"))
}

/// 从 `src/bin/` 目录推导模块路径，如 `src/bin/foo.rs`
pub fn from_bin_path(target: &Target, file_path: &Path) -> Option<String> {
    let bin_path = target.src_path().path()?;
    if bin_path == file_path {
        Some(target.name().to_string())
    } else {
        None
    }
}
