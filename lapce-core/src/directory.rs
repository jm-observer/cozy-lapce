use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use directories::{BaseDirs, ProjectDirs};

use crate::meta::NAME;
#[derive(Clone, Debug)]
pub struct Directory {
    pub home_dir:             Option<PathBuf>,
    pub project_dirs:         Option<ProjectDirs>,
    pub data_local_directory: Option<PathBuf>,
    pub logs_directory:       PathBuf,
    pub cache_directory:      Option<PathBuf>,
    pub proxy_directory:      PathBuf,
    pub themes_directory:     PathBuf,
    pub plugins_directory:    PathBuf,
    pub config_directory:     PathBuf,
    pub local_socket:         PathBuf,
    pub updates_directory:    Option<PathBuf>,
    pub queries_directory:    PathBuf,
    pub grammars_directory:   PathBuf
}

async fn init_path(dir: &Path) -> Result<()> {
    if !dir.exists() && dir.is_dir() {
        tokio::fs::create_dir_all(dir).await?;
    }
    Ok(())
}
async fn init_option_path(dir: &Option<PathBuf>) -> Result<()> {
    if let Some(dir) = dir {
        init_path(dir).await?;
    }
    Ok(())
}

impl Directory {
    pub async fn new() -> Result<Self> {
        let home_dir = Self::home_dir();
        init_option_path(&home_dir).await?;
        let project_dirs = Self::project_dirs();

        let data_local_directory = Self::data_local_directory_with_init().await;
        let logs_directory = Self::logs_directory_with_init()
            .await
            .ok_or(anyhow!("logs directory missing"))?;
        let cache_directory = Self::cache_directory_with_init().await;

        let proxy_directory = Self::proxy_directory_with_init()
            .await
            .ok_or(anyhow!("proxy directory missing"))?;
        let themes_directory = Self::themes_directory_with_init()
            .await
            .ok_or(anyhow!("themes directory missing"))?;
        let plugins_directory = Self::plugins_directory_with_init()
            .await
            .ok_or(anyhow!("plugins directory missing"))?;
        let config_directory = Self::config_directory_with_init()
            .await
            .ok_or(anyhow!("config directory missing"))?;
        let local_socket = Self::local_socket()
            .await
            .ok_or(anyhow!("local socket missing"))?;
        if !local_socket.exists() {
            tokio::fs::File::create(&local_socket).await?;
        }
        let updates_directory = Self::updates_directory_with_init().await;
        let queries_directory = Self::queries_directory_with_init()
            .await
            .ok_or(anyhow!("queries directory missing"))?;
        let grammars_directory = Self::grammars_directory_with_init()
            .await
            .ok_or(anyhow!("grammars directory missing"))?;
        Ok(Self {
            home_dir,
            project_dirs,
            data_local_directory,
            logs_directory,
            cache_directory,
            proxy_directory,
            themes_directory,
            plugins_directory,
            config_directory,
            local_socket,
            updates_directory,
            queries_directory,
            grammars_directory
        })
    }

    fn home_dir() -> Option<PathBuf> {
        BaseDirs::new().map(|d| PathBuf::from(d.home_dir()))
    }

    #[cfg(not(feature = "portable"))]
    fn project_dirs() -> Option<ProjectDirs> {
        ProjectDirs::from("dev", "lapce", NAME)
    }

    /// Return path adjacent to lapce executable when built as portable
    #[cfg(feature = "portable")]
    fn project_dirs() -> Option<ProjectDirs> {
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                return ProjectDirs::from_path(parent.join("lapce-data"));
            }
            unreachable!("Couldn't obtain current process parent path");
        }
        unreachable!("Couldn't obtain current process path");
    }

    // Get path of local data directory
    // Local data directory differs from data directory
    // on some platforms and is not transferred across
    // machines
    async fn data_local_directory_with_init() -> Option<PathBuf> {
        match Self::project_dirs() {
            Some(dir) => {
                let dir = dir.data_local_dir();
                if !dir.exists() {
                    if let Err(err) = tokio::fs::create_dir_all(dir).await {
                        log::error!("{:?}", err);
                    }
                }
                Some(dir.to_path_buf())
            },
            None => None
        }
    }

    /// Get the path to logs directory
    /// Each log file is for individual application startup
    async fn logs_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("logs");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    /// Get the path to cache directory
    async fn cache_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("cache");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    /// Directory to store proxy executables used on local
    /// host as well, as ones uploaded to remote host when
    /// connecting
    async fn proxy_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("proxy");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    /// Get the path to the themes folder
    /// Themes are stored within as individual toml files
    async fn themes_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("themes");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    // Get the path to plugins directory
    // Each plugin has own directory that contains
    // metadata file and plugin wasm
    async fn plugins_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("plugins");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    // Config directory contain only configuration files
    async fn config_directory_with_init() -> Option<PathBuf> {
        match Self::project_dirs() {
            Some(dir) => {
                let dir = dir.config_dir();
                if !dir.exists() {
                    if let Err(err) = tokio::fs::create_dir_all(dir).await {
                        log::error!("{:?}", err);
                    }
                }
                Some(dir.to_path_buf())
            },
            None => None
        }
    }

    async fn local_socket() -> Option<PathBuf> {
        Self::data_local_directory_with_init()
            .await
            .map(|dir| dir.join("local.sock"))
    }

    async fn updates_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("updates");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }
            Some(dir)
        } else {
            None
        }
    }

    async fn queries_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::config_directory_with_init().await {
            let dir = dir.join("queries");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }

            Some(dir)
        } else {
            None
        }
    }

    async fn grammars_directory_with_init() -> Option<PathBuf> {
        if let Some(dir) = Self::data_local_directory_with_init().await {
            let dir = dir.join("grammars");
            if !dir.exists() {
                if let Err(err) = tokio::fs::create_dir(&dir).await {
                    log::error!("{:?}", err);
                }
            }

            Some(dir)
        } else {
            None
        }
    }
}
