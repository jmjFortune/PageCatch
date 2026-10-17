use crate::{config::Config, resolver::Resolved};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;

async fn run(cmd: &mut Command, limit: Duration) -> Result<std::process::Output> {
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    tokio::time::timeout(limit, cmd.output())
        .await
        .context("Media verification timed out")?
        .context("Could not run the media tool")
}
pub async fn probe(config: &Config, path: &Path) -> Result<f64> {
    let o = run(
        Command::new(&config.ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration:stream=codec_type",
                "-of",
                "json",
            ])
            .arg(path),
        Duration::from_secs(20),
    )
    .await?;
    anyhow::ensure!(o.status.success(), "ffprobe could not read the output file");
    let v: Value = serde_json::from_slice(&o.stdout)?;
    anyhow::ensure!(
        v["streams"]
            .as_array()
            .is_some_and(|s| s.iter().any(|s| s["codec_type"] == "video")),
        "The file has no video track"
    );
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .context("No valid duration found")?;
    anyhow::ensure!(
        duration.is_finite() && duration > 0.0,
        "Invalid video duration"
    );
    Ok(duration)
}
/// 允许的时长缺口：基础容忍（3 秒或 1%），再加上明知缺片对应的那段时长。
/// 退选保存时缺片是已知的，不能再拿完整列表的时长去卡它，否则补救等于白做。
fn duration_slack(expected: f64, missing: Option<(u64, u64)>) -> f64 {
    let base = 3.0_f64.max(expected * 0.01);
    match missing {
        Some((missing, total)) if total > 0 => {
            base + expected * missing as f64 / total as f64 + 1.0
        }
        _ => base,
    }
}
pub async fn verify(
    config: &Config,
    id: i64,
    path: &Path,
    expected: Option<f64>,
    missing: Option<(u64, u64)>,
) -> Result<(f64, bool)> {
    anyhow::ensure!(
        tokio::fs::metadata(path).await?.len() > 1024,
        "The output file is too small"
    );
    let duration = probe(config, path).await?;
    if config.preview_segments.is_none()
        && let Some(expected) = expected
    {
        anyhow::ensure!(
            (duration - expected).abs() <= duration_slack(expected, missing),
            "Output duration {duration:.2}s does not match the source playlist duration {expected:.2}s"
        );
    }
    let o = run(
        Command::new(&config.ffmpeg)
            .args(["-v", "error", "-err_detect", "explode", "-xerror", "-i"])
            .arg(path)
            .args(["-f", "null", "-"]),
        config.timeout(),
    )
    .await?;
    let stderr = String::from_utf8_lossy(&o.stderr);
    if o.status.success() && stderr.is_empty() {
        return Ok((duration, false));
    }
    match crate::remedy::choose(crate::remedy::Failure::Decode {
        success: o.status.success(),
        stderr: &stderr,
    }) {
        crate::remedy::Remedy::KeepAsIs => {
            super::append_log(
                config,
                id,
                "Strict decoding verification found only a timestamp warning; keeping the video as-is",
            );
            Ok((duration, true))
        }
        _ => anyhow::bail!(
            "Strict decoding verification failed: {}",
            stderr.chars().take(600).collect::<String>()
        ),
    }
}
async fn hashes(
    config: &Config,
    input: &str,
    headers: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<Vec<String>> {
    let mut cmd = Command::new(&config.ffmpeg);
    cmd.args(["-v", "error"]);
    if let Some(h) = headers {
        let h = h
            .iter()
            .map(|(k, v)| format!("{k}: {v}\r\n"))
            .collect::<String>();
        cmd.args(["-headers", &h, "-allowed_extensions", "ALL"]);
    }
    cmd.args([
        "-i",
        input,
        "-t",
        "6",
        "-an",
        "-vf",
        "fps=25,scale=160:90,format=gray",
        "-f",
        "framemd5",
        "-",
    ]);
    let o = run(&mut cmd, Duration::from_secs(25)).await?;
    anyhow::ensure!(o.status.success(), "Could not extract source frames");
    let values = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next().map(|s| s.trim().to_owned()))
        .collect::<Vec<_>>();
    anyhow::ensure!(values.len() >= 50, "Not enough frames");
    Ok(values)
}
fn same_frames(a: &[String], b: &[String]) -> bool {
    // 避免纯黑/静止开场产生虚假的高一致率。
    if a.iter().collect::<std::collections::HashSet<_>>().len() < 20 {
        return false;
    }
    (-15i32..=15).any(|shift| {
        let pairs = a
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let j = i as i32 + shift;
                if j < 0 {
                    return None;
                }
                b.get(j as usize).map(|w| v == w)
            })
            .collect::<Vec<_>>();
        pairs.len() >= 50
            && pairs.iter().filter(|v| **v).count() as f64 / pairs.len() as f64 >= 0.95
    })
}
pub async fn duplicate(config: &Config, media: &Resolved) -> Result<Option<PathBuf>> {
    if config.preview_segments.is_some() {
        return Ok(None);
    }
    let Some(duration) = media.duration else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    for p in super::output::video_files(&config.output).await? {
        if let Ok(d) = probe(config, &p).await
            && (d - duration).abs() <= 3.0
        {
            candidates.push(p);
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    let source = match hashes(config, media.url.as_str(), Some(&media.headers)).await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    for path in candidates {
        if let Ok(local) = hashes(config, &path.to_string_lossy(), None).await
            && same_frames(&source, &local)
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_slack_grows_for_known_missing_segments() {
        // 完整列表：只给基础容忍。
        assert_eq!(duration_slack(1206.0, None), 12.06);
        assert_eq!(duration_slack(100.0, None), 3.0);
        // 明知缺 1/753：多给对应时长，否则补救保存会卡在时长检查上。
        assert!(duration_slack(1206.0, Some((1, 753))) > 13.6);
        assert_eq!(duration_slack(1206.0, Some((0, 0))), 12.06);
    }
    #[test]
    fn matching_requires_motion_and_alignment() {
        let a = (0..150).map(|i| format!("frame-{i}")).collect::<Vec<_>>();
        let b = a[5..].to_vec();
        assert!(same_frames(&a, &b));
        assert!(!same_frames(&a, &vec!["other".into(); 150]));
        assert!(!same_frames(
            &vec!["black".into(); 150],
            &vec!["black".into(); 150]
        ));
    }
}
