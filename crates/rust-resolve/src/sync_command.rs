use std::{
    io::{BufRead, BufReader},
    process::Command,
    thread,
};

use anyhow::{anyhow, bail};
use cozy_floem::{channel::ExtChannel, views::tree_with_panel::data::StyledText};
use log::{error, info};

use crate::{resolve_stderr, resolve_stdout};

pub fn run_command(
    mut command: Command,
    mut channel: ExtChannel<StyledText>,
) -> anyhow::Result<()> {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start cargo build");
    let stdout = child.stdout.take().ok_or(anyhow!("stdout is none"))?;
    let stderr = child.stderr.take().ok_or(anyhow!("stderr is none"))?;

    let mut out_lines = BufReader::new(stdout).lines();
    let mut err_lines = BufReader::new(stderr).lines();
    let mut sync_channel = channel.clone();
    let out_thread = thread::spawn(move || {
        while let Some(Ok(line)) = out_lines.next() {
            if let Some(text) = resolve_stdout(&line) {
                sync_channel.send(text);
            }
        }
    });
    let err_thread = thread::spawn(move || {
        while let Some(Ok(line)) = err_lines.next() {
            channel.send(resolve_stderr(&line));
        }
    });

    if let Err(err) = out_thread.join() {
        error!("{err:?}");
    }
    if let Err(err) = err_thread.join() {
        error!("{err:?}");
    }

    let status = child.wait()?;
    if !status.success() {
        bail!("child process failed: {}", status);
    }
    info!("run end");
    Ok(())
}
