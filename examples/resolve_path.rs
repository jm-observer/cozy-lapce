use std::path::PathBuf;

use lapce_rpc::rust_module_resolve::create_cargo_context;
use log::debug;

fn main() -> anyhow::Result<()> {
    custom_utils::logger::logger_stdout(
        "warn,resolve_path=debug,lapce_proxy::rust_module_resolve=debug",
    );
    let manifest_path: PathBuf = "D:\\git\\cozy-lapce\\Cargo.toml".into();
    let cargo = create_cargo_context(&manifest_path)?;

    // for package in cargo.workspace.members() {
    //     debug!("{package:?}");
    //     for target in package.targets() {
    //         debug!("{target:?}")
    //     }
    // }
    {
        let file_path: PathBuf =
            "D:\\git\\cozy-lapce\\examples\\resolve_path.rs".into();
        debug!("{:?}", cargo.file_path_to_module_path(&file_path));
    }
    {
        let file_path: PathBuf =
            "D:\\git\\cozy-lapce\\lapce-app\\src\\bin\\lapce.rs".into();
        debug!("{:?}", cargo.file_path_to_module_path(&file_path));
    }
    {
        let file_path: PathBuf =
            "D:\\git\\cozy-lapce\\lapce-app\\src\\common\\mod.rs".into();
        debug!("{:?}", cargo.file_path_to_module_path(&file_path));
        let file_path: PathBuf =
            "D:\\git\\cozy-lapce\\lapce-app\\src\\common\\head.rs".into();
        debug!("{:?}", cargo.file_path_to_module_path(&file_path));
    }
    {
        let file_path: PathBuf =
            "D:\\git\\cozy-lapce\\lapce-app\\tests\\test_all.rs".into();
        debug!("{:?}", cargo.file_path_to_module_path(&file_path));
    }
    Ok(())
}
