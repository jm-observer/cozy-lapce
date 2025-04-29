use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, bail};
use cargo::{
    core::{PackageSet, Resolve, Shell, Workspace},
    ops,
    util::GlobalContext,
};
use directories::UserDirs;
use lapce_rpc::{RpcResult, proxy::FileAndLine};
use log::{error, warn};

fn cargo_home() -> Result<PathBuf> {
    Ok(if let Ok(path) = std::env::var("CARGO_HOME") {
        PathBuf::from(path)
    } else if let Some(user_dir) = UserDirs::new() {
        user_dir.home_dir().join(".cargo")
    } else {
        bail!("Cannot determine CARGO_HOME");
    })
}

pub struct CargoContext {
    pub gctx:      Arc<GlobalContext>,
    pub workspace: Workspace<'static>,
    pub resolve:   Resolve,
    pub packages:  PackageSet<'static>,
}

pub fn create_cargo_context(manifest_path: &Path) -> anyhow::Result<CargoContext> {
    // GlobalContext
    let gctx = Arc::new(GlobalContext::new(
        Shell::new(),
        std::env::current_dir()?,
        cargo_home()?,
    ));
    // 这里 leak 成 'static 生命周期
    let leaked_gctx: &'static GlobalContext = unsafe {
        let raw = Arc::into_raw(gctx.clone());
        &*raw
    };
    // Workspace
    let workspace = Workspace::new(manifest_path, leaked_gctx)?;
    let (packages, resolve) = ops::resolve_ws(&workspace, true)?;
    // let workspace = Arc::new(RwLock::new(workspace));
    // let resolve = Arc::new(resolve);
    // let package_set = Arc::new(packages);

    Ok(CargoContext {
        gctx,
        workspace,
        resolve,
        packages,
    })
}

impl CargoContext {
    pub fn find_file_by_location(
        &self,
        location_info: &LocationInfo,
    ) -> RpcResult<FileAndLine> {
        if let Ok(file) = self.find_path_of_package(location_info) {
            // if !path.exists() {
            //     log::error!("{location_info:?} path not exists {path:?}");
            //     return None;
            // }
            return RpcResult::Ok(FileAndLine {
                file,
                line: location_info.line,
            });
        }
        match self.find_path_of_bin_or_example(&location_info.krate) {
            Ok(file) => RpcResult::Ok(FileAndLine {
                file,
                line: location_info.line,
            }),
            Err(error) => RpcResult::Err(error.to_string()),
        }
    }

    pub fn find_file_by_log(&self, log: &str) -> RpcResult<FileAndLine> {
        let Some(location) = parse_location(log) else {
            return RpcResult::Err(format!("{log} parse to location fail"));
        };
        self.find_file_by_location(&location)
    }

    fn find_path_of_package(&self, location_info: &LocationInfo) -> Result<PathBuf> {
        let id = match self.resolve.query(&location_info.krate) {
            Ok(id) => id,
            Err(_) => {
                if location_info.krate.contains("_") {
                    self.resolve.query(&location_info.krate.replace('_', "-"))?
                } else {
                    bail!("`{}` did not match any packages", location_info.krate);
                }
            },
        };
        let package = self.packages.get_one(id)?;
        let mut file_path = package.root().join("src");
        for part in
            &location_info.modules[..location_info.modules.len().saturating_sub(1)]
        {
            file_path = file_path.join(part);
        }

        if let Some(last) = location_info.modules.last() {
            let candidate1 = file_path.join(format!("{}.rs", last));
            let candidate2 = file_path.join(last).join("mod.rs");

            if candidate1.exists() {
                Ok(candidate1)
            } else if candidate2.exists() {
                Ok(candidate2)
            } else {
                error!("Neither file nor mod.rs found for module: {:?}", file_path);
                bail!("Neither file nor mod.rs found for module: {:?}", file_path);
            }
        } else {
            file_path = file_path.join("lib.rs");
            if file_path.exists() {
                Ok(file_path)
            } else {
                // maybe log in main.rs
                warn!("lib.rs found: {:?}", file_path);
                bail!("lib.rs found: {:?}", file_path);
            }
        }
    }

    fn find_path_of_bin_or_example(&self, krate: &str) -> Result<PathBuf> {
        for package in self.workspace.members() {
            for target in package.targets() {
                if target.crate_name() == krate {
                    if let Some(path) = target.src_path().path() {
                        return Ok(path.to_path_buf());
                    }
                }
            }
        }
        bail!("`{}` did not match any bin/example/package", krate)
    }
}

#[derive(Debug)]
pub struct LocationInfo {
    krate:   String,
    modules: Vec<String>,
    line:    u32,
}

fn parse_location(s: &str) -> Option<LocationInfo> {
    let (module_path, line_str) = s.rsplit_once(':')?;

    let (krate, modules) = match module_path.split_once("::") {
        Some((krate, modules_path)) => {
            let modules = modules_path
                .split("::")
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            (krate.to_string(), modules)
        },
        None => {
            // 没有 "::"，说明只有 crate 名
            (module_path.to_string(), Vec::new())
        },
    };

    let line = line_str.parse::<u32>().ok()?;

    Some(LocationInfo {
        krate: krate.to_string(),
        modules,
        line,
    })
}
