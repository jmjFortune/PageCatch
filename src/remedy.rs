//! 失败分类与补救选择。一次失败该走哪种补救只在这里判定，别处不再各写一套判断，
//! 这样策略集中、可测、可审计。
//!
//! 代价从低到高，永远先挑代价低的：
//!
//! 1. [`Remedy::KeepAsIs`] 无损、零代价——失败其实没坏，例如解码只报了容器时间戳提示。
//! 2. [`Remedy::Stop`] 零代价——重试拿到的是同样的结果，例如媒体域名根本解析不了。
//! 3. [`Remedy::AllowMissingSegments`] 要重跑一次引擎——无损，但成品缺一小段。
//! 4. [`Remedy::RetryLater`] 交给队列按既有退避再试一次——来源不明、可能是瞬时的问题。
//!
//! 刻意不做的事：重编视频（有损，且会把源站的坏数据变成"看起来正常"）、为丢片去换
//! 线路重下（收益还没在真站点上验证过）。

use std::sync::LazyLock;

/// 判定补救所需的最少信息。
#[derive(Debug)]
pub enum Failure<'a> {
    /// 下载引擎失败：`log` 是本次尝试追加的那段日志。
    Engine(&'a str),
    /// 严格解码校验：`success` 是退出码，`stderr` 是它的输出。
    Decode { success: bool, stderr: &'a str },
    /// 一次尝试的整体失败信息（解析、下载、校验都会走到这里）。
    Message(&'a str),
}

/// 选定的补救。
#[derive(Debug, PartialEq, Eq)]
pub enum Remedy {
    /// 原样保留这份成品。
    KeepAsIs,
    /// 允许缺片再下一次引擎，成品会缺一小段。
    AllowMissingSegments { missing: u64, total: u64 },
    /// 这次尝试失败，但值得让队列按退避再试。
    RetryLater,
    /// 放弃，别再重试，并记下原因。
    Stop(&'static str),
}

/// 引擎明确报告“播放列表列出的分片数与实际拿到的不符”时的原文。
static SEGMENT_COUNT: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"Segment count check not pass, total: (\d+), downloaded: (\d+)")
        .expect("segment count pattern")
});
/// 域名解析不了：名字不会在退避的几秒里复活，重复尝试没有意义。
fn dns_failure(error: &str) -> bool {
    error.contains("dns error")
        || error.contains("ERR_NAME_NOT_RESOLVED")
        || error.contains("Name or service not known")
}
/// 空行之外，只剩时间戳提示：解码本身没有出错。
fn only_timestamp_warnings(stderr: &str) -> bool {
    let mut lines = stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .peekable();
    if lines.peek().is_none() {
        return false;
    }
    lines.all(|line| {
        line.contains("non monotonically increasing dts") || line.contains("Non-monotonous DTS")
    })
}
impl Remedy {
    /// 记日志用的短名。
    pub fn label(&self) -> &'static str {
        match self {
            Remedy::KeepAsIs => "keep as-is",
            Remedy::AllowMissingSegments { .. } => "allow missing segments",
            Remedy::RetryLater => "retry later",
            Remedy::Stop(_) => "stop",
        }
    }
}
pub fn choose(failure: Failure<'_>) -> Remedy {
    match failure {
        Failure::Decode { success, stderr } => {
            if success && only_timestamp_warnings(stderr) {
                Remedy::KeepAsIs
            } else if success {
                // 退出码 0，但有认不出来的输出：当成真正的解码问题，重下可能拿到干净的。
                Remedy::RetryLater
            } else {
                Remedy::RetryLater
            }
        }
        Failure::Engine(log) => match SEGMENT_COUNT.captures(log) {
            Some(captures) => {
                let total: u64 = captures[1].parse().unwrap_or(0);
                let downloaded: u64 = captures[2].parse().unwrap_or(0);
                if total > downloaded {
                    Remedy::AllowMissingSegments {
                        missing: total - downloaded,
                        total,
                    }
                } else {
                    Remedy::RetryLater
                }
            }
            // 认不出来的引擎失败：网络、鉴权、解密、超时都可能，值得按退避再试。
            None => Remedy::RetryLater,
        },
        Failure::Message(error) => {
            if dns_failure(error) {
                Remedy::Stop("The media host does not resolve; retrying would fail the same way")
            } else {
                Remedy::RetryLater
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_segment_becomes_a_allowed_gap() {
        let log = "ERROR: Response status code does not indicate success: 404 (Not Found).\n\
                   ERROR: Segment count check not pass, total: 753, downloaded: 752.\n\
                   ERROR: Failed\n";
        assert_eq!(
            choose(Failure::Engine(log)),
            Remedy::AllowMissingSegments {
                missing: 1,
                total: 753
            }
        );
        // 没有“分片数不符”这句话的引擎失败不退选，交给退避重试。
        assert_eq!(
            choose(Failure::Engine("ERROR: Failed to get the key\n")),
            Remedy::RetryLater
        );
    }
    #[test]
    fn a_timestamp_warning_keeps_the_video_others_retry() {
        let warning = "[null @ 0x55] Application provided invalid, non monotonically increasing dts to muxer in stream 0: 3665 >= 3665\n";
        assert_eq!(
            choose(Failure::Decode {
                success: true,
                stderr: warning
            }),
            Remedy::KeepAsIs
        );
        assert_eq!(
            choose(Failure::Decode {
                success: true,
                stderr: &format!("{warning}[h264 @ 0x1] Invalid NAL unit size\n")
            }),
            Remedy::RetryLater
        );
        assert_eq!(
            choose(Failure::Decode {
                success: false,
                stderr: warning
            }),
            Remedy::RetryLater
        );
    }
    #[test]
    fn an_unresolvable_host_stops_the_retries() {
        assert!(matches!(
            choose(Failure::Message(
                "No valid media playlist found. error sending request for url (https://x/y.m3u8): \
                 error trying to connect: dns error: failed to lookup address information"
            )),
            Remedy::Stop(_)
        ));
        assert_eq!(
            choose(Failure::Message("No valid media playlist found.")),
            Remedy::RetryLater
        );
    }
}
