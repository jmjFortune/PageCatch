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

/// 判为「再重试也不会变」的失败。队列据此停止退避重试；用类型而不是文本传递，
/// 免得两边靠字符串约定耦合。
#[derive(Debug)]
pub struct Fatal(pub &'static str);
impl std::fmt::Display for Fatal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Fatal {}

/// 判定补救所需的最少信息。
#[derive(Debug)]
pub enum Failure<'a> {
    /// 下载引擎失败：`log` 是本次尝试追加的那段日志。
    Engine(&'a str),
    /// 严格解码校验：`success` 是退出码，`stderr` 是它的输出，`tail_damage` 是
    /// 「丢掉结尾若干秒就能解码干净」的秒数（调用方先取证，坏在哪儿由这里判定）。
    Decode {
        success: bool,
        stderr: &'a str,
        tail_damage: Option<f64>,
    },
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
/// 引擎连播放列表都没拿到就被拒（403/404/410）：地址本身已失效，签名再新也没用
/// ——换一次重试只会拿到同样被拒的地址。分片级的 403/404 不算，那时列表已经解析过。
fn playlist_refused(log: &str) -> bool {
    let refused = ["403 (Forbidden)", "404 (Not Found)", "410 (Gone)"]
        .iter()
        .any(|code| log.contains(code));
    refused && !log.contains("Extracted, there are")
}
/// 空行之外，只剩时间戳提示：解码本身没有出错。
pub fn only_timestamp_warnings(stderr: &str) -> bool {
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
        Failure::Decode {
            success,
            stderr,
            tail_damage,
        } => {
            if success && only_timestamp_warnings(stderr) {
                Remedy::KeepAsIs
            } else if tail_damage.is_some() {
                // 损坏只在结尾（源站最后一个分片被截断）：整片可播，丢掉结尾几秒
                // 比整条失败划算。
                Remedy::KeepAsIs
            } else {
                // 其余解码失败（退出码非 0、或坏在片中）：重下可能拿到干净的。
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
            None if playlist_refused(log) => {
                Remedy::Stop("The playlist address is dead: the server refuses it (403/404)")
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
                stderr: warning,
                tail_damage: None
            }),
            Remedy::KeepAsIs
        );
        assert_eq!(
            choose(Failure::Decode {
                success: true,
                stderr: &format!("{warning}[h264 @ 0x1] Invalid NAL unit size\n"),
                tail_damage: None
            }),
            Remedy::RetryLater
        );
        assert_eq!(
            choose(Failure::Decode {
                success: false,
                stderr: warning,
                tail_damage: None
            }),
            Remedy::RetryLater
        );
    }
    #[test]
    fn damage_confined_to_the_tail_keeps_the_video() {
        // 源站最后一个分片被截断：丢掉结尾几秒能解码干净，就保留整片。
        let aac = "[aac @ 0x55] Input buffer exhausted before END element found\n";
        assert_eq!(
            choose(Failure::Decode {
                success: false,
                stderr: aac,
                tail_damage: Some(5.0)
            }),
            Remedy::KeepAsIs
        );
        // 取证说坏的地方不止结尾，就照旧重试。
        assert_eq!(
            choose(Failure::Decode {
                success: false,
                stderr: aac,
                tail_damage: None
            }),
            Remedy::RetryLater
        );
    }
    #[test]
    fn a_refused_playlist_stops_the_retries() {
        // 引擎连列表都没拿到就被拒：地址已失效，换新签名也一样。
        let log = "Loading URL: https://cdn.example/a/index.m3u8?sign=x\n\
                   WARN : Response status code does not indicate success: 403 (Forbidden). (10/10)\n";
        assert!(matches!(choose(Failure::Engine(log)), Remedy::Stop(_)));
        // 分片级的 403 不算这一类：列表已经解析过了，重试可能有用。
        let segments = "Loading URL: https://cdn.example/a/index.m3u8\n\
                        INFO : Extracted, there are 1 streams\n\
                        WARN : Response status code does not indicate success: 403 (Forbidden).\n";
        assert_eq!(choose(Failure::Engine(segments)), Remedy::RetryLater);
    }
    #[test]
    fn an_unresolvable_host_stops_the_retries() {
        // 解析器现在会把错误链带上（`{e:#}`），真实文本长这样——补救表就靠这层原因判断。
        assert!(matches!(
            choose(Failure::Message(
                "No valid media playlist found. error sending request for url \
                 (https://xhspp.sdzxhq.com/20250622/9bmlaWfI/index.m3u8): error trying to \
                 connect: dns error: failed to lookup address information: Name or service not known"
            )),
            Remedy::Stop(_)
        ));
        assert!(matches!(
            choose(Failure::Message(
                "No valid media playlist found. Browser parsing: Desktop: No media requests \
                 detected after loading the page. Media candidates failed: \
                 https://x.example/a.m3u8 (net::ERR_NAME_NOT_RESOLVED)"
            )),
            Remedy::Stop(_)
        ));
        assert_eq!(
            choose(Failure::Message("No valid media playlist found.")),
            Remedy::RetryLater
        );
    }
}
