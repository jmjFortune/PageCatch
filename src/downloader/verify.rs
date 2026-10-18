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
) -> Result<(f64, Option<String>)> {
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
        return Ok((duration, None));
    }
    let success = o.status.success();
    // 取证：损坏是不是只在结尾（源站最后一个分片被截断）。时间戳提示那类不必取证。
    let tail_damage = if success && crate::remedy::only_timestamp_warnings(&stderr) {
        None
    } else {
        tail_damage_seconds(config, path, duration).await?
    };
    let choice = crate::remedy::choose(crate::remedy::Failure::Decode {
        success,
        stderr: &stderr,
        tail_damage,
    });
    if choice == crate::remedy::Remedy::KeepAsIs {
        let note = match tail_damage {
            Some(seconds) => {
                format!("the source is truncated; kept everything but the last {seconds:.0} s")
            }
            None => "kept as-is with a timestamp warning".to_owned(),
        };
        super::append_log(config, id, &format!("Strict decoding verification: {note}"));
        return Ok((duration, Some(note)));
    }
    anyhow::bail!(
        "Strict decoding verification failed: {}",
        stderr.chars().take(600).collect::<String>()
    )
}
/// 解码失败时取证：源站最后一个分片被截断是常见缺陷；“丢掉结尾若干秒就能解码干净”
/// 说明坏的地方局限在尾部，整片保下来比整条失败划算。返回丢掉的秒数。
async fn tail_damage_seconds(config: &Config, path: &Path, duration: f64) -> Result<Option<f64>> {
    for drop in [5.0_f64, 30.0] {
        if duration <= drop + 1.0 {
            continue;
        }
        let keep = duration - drop;
        let o = run(
            Command::new(&config.ffmpeg)
                .args(["-v", "error", "-err_detect", "explode", "-xerror", "-i"])
                .arg(path)
                .args(["-t", &format!("{keep:.3}"), "-f", "null", "-"]),
            config.timeout(),
        )
        .await?;
        if o.status.success() && o.stderr.is_empty() {
            return Ok(Some(drop));
        }
    }
    Ok(None)
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
    use clap::Parser;
    /// 结尾被截断的成品要能救回来，坏在片中的不能——用真实 ffmpeg 各验一遍。
    #[tokio::test]
    async fn a_video_truncated_at_the_end_is_kept_but_mid_damage_is_not() {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            eprintln!("skipped: ffmpeg not available");
            return; // 没装 ffmpeg 就跳过（镜像构建阶段就是这种情形）
        }
        let dir = tempfile::tempdir().unwrap();
        let whole = dir.path().join("whole.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=160x120:rate=25:duration=12",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=12",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
                "-shortest",
            ])
            .arg(&whole)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !made {
            eprintln!("skipped: ffmpeg could not encode the test video");
            return; // 本机 ffmpeg 没带 libx264/aac 也跳过
        }
        let mut config = Config::parse_from(["pagecatch"]);
        config.ffmpeg = "ffmpeg".into();
        config.ffprobe = "ffprobe".into();
        config.data = dir.path().to_path_buf();
        let bytes = std::fs::read(&whole).unwrap();

        // 1) 尾部被截断：救回来，并说明丢掉了结尾多少秒。
        let cut = dir.path().join("cut.mp4");
        std::fs::write(&cut, &bytes[..bytes.len() - 3000]).unwrap();
        let (duration, note) = verify(&config, 1, &cut, None, None)
            .await
            .expect("结尾截断应该被救回");
        assert!(duration > 0.0);
        assert!(
            note.unwrap_or_default().contains("last"),
            "说明里要写清楚丢的是结尾"
        );

        // 2) 坏在片中：不是结尾的问题，照旧判失败。
        let mid = dir.path().join("mid.mp4");
        let mut damaged = bytes.clone();
        let offset = damaged.len() / 2;
        damaged[offset..offset + 2000].fill(0);
        std::fs::write(&mid, &damaged).unwrap();
        assert!(
            verify(&config, 2, &mid, None, None).await.is_err(),
            "坏在片中的成品不该被放行"
        );
    }
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
