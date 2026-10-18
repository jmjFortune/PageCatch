pub(super) mod output;
pub mod verify;
use crate::{
    config::Config, model::Task, remedy, remedy::Remedy, resolver::Resolved, store::Store,
};
use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

struct DownloadProcess {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    exited: bool,
}
impl DownloadProcess {
    fn stop(&mut self) {
        if self.exited {
            return;
        }
        if let Some(pid) = self.child.process_id() {
            #[cfg(unix)]
            // portable-pty 在独立 session 中启动子进程；同时终止引擎及其 FFmpeg 子进程。
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exited = true;
    }
}
impl Drop for DownloadProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn log_path(config: &Config, id: i64) -> PathBuf {
    config.data.join("logs").join(format!("{id}.log"))
}
pub fn dependency_check(config: &Config) -> Result<()> {
    for (name, exe, arg) in [
        ("Download engine", config.downloader.as_str(), "--version"),
        ("FFmpeg", config.ffmpeg.as_str(), "-version"),
        ("ffprobe", config.ffprobe.as_str(), "-version"),
    ] {
        let result = std::process::Command::new(exe)
            .arg(arg)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .with_context(|| {
                format!("Could not find {name}: {exe}. Use the Docker image with bundled dependencies or set the PC_* paths.")
            })?;
        anyhow::ensure!(result.success(), "Could not run {name}: {exe}");
    }
    Ok(())
}
fn total_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| {
            if e.path().is_dir() {
                total_size(&e.path())
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}
/// 下载结果：成品文件，以及退选保存时缺了多少分片。
pub struct Downloaded {
    pub path: PathBuf,
    /// (缺失数, 总片数)：只有退选保存时才有值。
    pub missing_segments: Option<(u64, u64)>,
}
/// 跑一次下载引擎。allow_missing_segments 只在退选重跑时打开。
///
/// PTY 同时覆盖 Unix 和 Windows ConPTY，避免下载引擎无终端时的进度输出崩溃。
#[allow(clippy::too_many_arguments)]
fn run_engine(
    config: &Config,
    task: &Task,
    media: &Resolved,
    store: &Store,
    cancel: &CancellationToken,
    work: &Path,
    allow_missing_segments: bool,
) -> Result<portable_pty::ExitStatus> {
    let mut cmd = CommandBuilder::new(&config.downloader);
    cmd.arg(media.url.as_str());
    cmd.args([
        "--save-dir",
        work.to_str()
            .context("Temporary directory path is not valid UTF-8")?,
        "--tmp-dir",
        work.to_str().unwrap(),
        "--save-name",
        "video",
        "--auto-select",
        "--no-log",
        "--del-after-done",
        "--disable-update-check",
        "--write-meta-json",
        "false",
        "--ffmpeg-binary-path",
        &config.ffmpeg,
        "-M",
        "format=mp4",
    ]);
    cmd.args([
        "--thread-count",
        &config.threads.to_string(),
        "--download-retry-count",
        "3",
    ]);
    if allow_missing_segments {
        // 退选：源站确实丢了分片时也合并出成品，视频里会缺一小段。
        cmd.args(["--check-segments-count", "false"]);
    }
    if let Some(n) = config.preview_segments {
        cmd.args(["--custom-range", &format!("0-{}", n - 1)]);
    }
    for (k, v) in &media.headers {
        cmd.args(["-H", &format!("{k}: {v}")]);
    }
    let pair = native_pty_system().openpty(PtySize {
        rows: 30,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let mut process = DownloadProcess {
        child: pair.slave.spawn_command(cmd)?,
        exited: false,
    };
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    // 不保留 PTY 的输入写端，下载不需要交互。
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(config, task.id))?;
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut written = 0;
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }
            if written < 8 * 1024 * 1024 {
                let _ = log.write_all(&buffer[..n]);
                written += n;
            }
        }
    });
    let start = Instant::now();
    let mut last = Instant::now();
    let exit = loop {
        if cancel.is_cancelled() || start.elapsed() > config.timeout() {
            process.stop();
            drop(pair.master);
            let _ = reader_thread.join();
            anyhow::bail!(if cancel.is_cancelled() {
                "Task cancelled"
            } else {
                "Download timed out"
            });
        }
        if let Some(status) = process.child.try_wait()? {
            process.exited = true;
            break status;
        }
        if last.elapsed() > Duration::from_secs(1) {
            let bytes = total_size(work);
            store.update(
                task.id,
                "downloading",
                "Downloading media segments",
                task.attempt,
                bytes,
                None,
            )?;
            last = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    drop(pair.master);
    let _ = reader_thread.join();
    Ok(exit)
}
/// 只读这次运行追加的那段日志，免得拿上一次尝试的失败原因来做判断。
fn read_log_tail(log: &Path, from: u64) -> Option<String> {
    let mut file = std::fs::File::open(log).ok()?;
    if file.metadata().ok()?.len() <= from {
        return None;
    }
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(from)).ok()?;
    let mut text = String::new();
    file.take(1 << 20).read_to_string(&mut text).ok()?;
    Some(text)
}
pub async fn download(
    config: Arc<Config>,
    task: Task,
    media: Resolved,
    store: Arc<Store>,
    cancel: CancellationToken,
) -> Result<Downloaded> {
    tokio::task::spawn_blocking(move || {
        let work = config.data.join("tmp").join(task.id.to_string());
        if work.exists() {
            std::fs::remove_dir_all(&work)?;
        }
        std::fs::create_dir_all(&work)?;
        let log = log_path(&config, task.id);
        let offset = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
        // 先严格跑一次：缺片、缺时长这类不完整成品照旧算失败。
        let mut exit = run_engine(&config, &task, &media, &store, &cancel, &work, false)?;
        let mut saved_missing = None;
        // 引擎失败按原因分类，各走各的补救（remedy 模块）。
        if !exit.success()
            && let Some(tail) = read_log_tail(&log, offset)
        {
            match remedy::choose(remedy::Failure::Engine(&tail)) {
                // 源站确实丢片：退选允许缺片再跑一次，成品会缺一小段。
                Remedy::AllowMissingSegments { missing, total } => {
                    append_log(
                        &config,
                        task.id,
                        &format!(
                            "The playlist lists {missing} of {total} segments the server does not serve; retrying once with the segment count check off"
                        ),
                    );
                    exit = run_engine(&config, &task, &media, &store, &cancel, &work, true)?;
                    if exit.success() {
                        saved_missing = Some((missing, total));
                    }
                }
                // 地址已失效：再重试只会拿到同样被拒的地址，直接停。
                Remedy::Stop(reason) => {
                    append_log(
                        &config,
                        task.id,
                        &format!("Remedy: stop retrying — {reason}"),
                    );
                    return Err(anyhow::Error::new(remedy::Fatal(reason)));
                }
                other => append_log(&config, task.id, &format!("Remedy: {}", other.label())),
            }
        }
        anyhow::ensure!(exit.success(), "Download engine failed. Check the task logs.");
        let target = work.join("video.mp4");
        anyhow::ensure!(target.is_file(), "Downloader did not produce an MP4 file");
        Ok(Downloaded {
            path: target,
            missing_segments: saved_missing,
        })
    })
    .await?
}
pub fn append_log(config: &Config, id: i64, message: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(config, id))
    {
        let _ = writeln!(f, "\n[PageCatch] {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_log_tail_only_covers_the_current_run() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("1.log");
        std::fs::write(
            &log,
            "Segment count check not pass, total: 3, downloaded: 2.\n",
        )
        .unwrap();
        assert!(read_log_tail(&log, 0).unwrap().contains("total: 3"));
        // 只有偏移之后追加的内容才算这次尝试的输出。
        let offset = std::fs::metadata(&log).unwrap().len();
        assert_eq!(read_log_tail(&log, offset), None);
        let mut file = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
        std::io::Write::write_all(&mut file, b"ERROR: Failed\n").unwrap();
        assert_eq!(read_log_tail(&log, offset).unwrap().trim(), "ERROR: Failed");
        assert_eq!(read_log_tail(&dir.path().join("missing.log"), 0), None);
    }
}
