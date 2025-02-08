use anyhow::Result;
use cozy_floem::{channel::ExtChannel, views::tree_with_panel::data::StyledText};
use log::info;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc
};

use crate::{resolve_stderr, resolve_stdout};

pub enum OutputLine {
    StdOut(String),
    StdErr(String)
}

pub async fn run_command(
    mut command: Command,
    mut channel: ExtChannel<StyledText>
) -> Result<()> {
    // 启动子进程，并捕获 stdout 和 stderr
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start cargo build");

    let (tx, mut rx) = mpsc::channel(100);

    // 异步读取 stdout
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(OutputLine::StdOut(line)).await.is_err() {
                    break;
                }
            }
        });
    }

    // 异步读取 stderr
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(OutputLine::StdErr(line)).await.is_err() {
                    break;
                }
            }
        });
    }

    drop(tx); // 关闭发送端，确保任务结束后 `rx` 能正确完成
    while let Some(message) = rx.recv().await {
        match message {
            OutputLine::StdOut(line) => {
                if let Some(text) = resolve_stdout(&line) {
                    channel.send(text);
                }
            },
            OutputLine::StdErr(line) => {
                channel.send(resolve_stderr(&line));
            }
        }
    }

    child.wait().await?;
    info!("child done");
    Ok(())
}
