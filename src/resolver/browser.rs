use crate::config::Config;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::Command,
};
use tokio_tungstenite::{WebSocketStream, connect_async, tungstenite::Message};
use url::Url;

pub fn executable(config: &Config) -> Option<PathBuf> {
    if let Some(ref p) = config.browser {
        return Some(p.clone());
    }
    let mut paths = vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
    ];
    for env in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Ok(p) = std::env::var(env) {
            paths.push(PathBuf::from(p).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    paths.into_iter().find(|p| p.is_file())
}

const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
#[cfg(test)]
const PLAYLIST: &str = "#EXTM3U\n#EXTINF:2,\nsegment.ts\n#EXT-X-ENDLIST\n";

/// 有些站点只给桌面浏览器一个跳向搜索引擎的脚本，对手机浏览器才给播放页。
const SEARCH_HOSTS: [&str; 7] = [
    "baidu.com",
    "google.",
    "bing.com",
    "so.com",
    "sogou.com",
    "haosou.com",
    "sm.cn",
];

enum CaptureResult {
    Media(Vec<(Url, BTreeMap<String, String>)>),
    MobileOnly,
    NoMedia {
        reason: String,
        /// 页面自己发起过媒体请求：说明这就是播放器页，失败在网络或源站。
        attempted_media: bool,
    },
}

fn is_search_engine(host: &str) -> bool {
    let host = host.to_lowercase();
    SEARCH_HOSTS
        .iter()
        .any(|s| host == *s || host.ends_with(&format!(".{s}")))
}

fn looks_like_media(url: &str, mime: &str) -> bool {
    let mime = mime.to_lowercase();
    mime.contains("mpegurl")
        || mime.contains("dash+xml")
        || url.contains(".m3u8")
        || url.contains(".mpd")
}

/// 详情页通常只放一个「立即播放」入口，播放器要进去才创建。找出站内的
/// 播放入口或播放控件：优先跟随入口链接，其次在控件上派发真实鼠标点击。
const PLAY_ENTRY_JS: &str = r#"(() => {
  if (document.querySelector('video')) return null;
  const hint = /立即播放|马上播放|点击播放|开始播放|播放|观看|play|watch/i;
  const strong = /立即播放|马上播放|点击播放|开始播放|play\s*now|watch\s*now/i;
  const blocked = /广告|ad[s_-]|advert|promo|banner|下载|download|直播|棋牌|娱乐|注册|充值/i;
  const nav = /header|menu|nav|footer|breadcrumb|tab|pager|page-jump|logo/i;
  const shown = el => {
    const r = el.getBoundingClientRect();
    if (r.width < 8 || r.height < 8 || r.top < -1 || r.top > innerHeight * 4) return false;
    const s = getComputedStyle(el);
    return s.visibility !== 'hidden' && s.display !== 'none' && s.opacity !== '0';
  };
  const mark = el => ((el.textContent || '') + ' ' + (el.className || '') + ' ' + (el.id || '')).trim();
  const score = t => (strong.test(t) ? 3 : /^(播放|观看|play|watch)$/i.test(t.trim()) ? 2 : hint.test(t) ? 1 : 0);
  const off = el => blocked.test(mark(el)) || (el.closest && el.closest('header,nav,footer,.header,.nav,.menu,.footer'));
  let link = null;
  for (const el of document.querySelectorAll('a[href]')) {
    if (!shown(el) || off(el)) continue;
    const t = mark(el);
    if (nav.test(t)) continue;
    const s = score(t);
    if (!s || (link && link.s >= s)) continue;
    try {
      const u = new URL(el.href, location.href);
      if (u.origin === location.origin && u.href !== location.href) link = {navigate: u.href, s};
    } catch (e) {}
  }
  if (link) return {navigate: link.navigate};
  let best = null;
  for (const sel of ['.btn-play', '[class*="btn-play" i]', 'video', '[class*="play" i]', '[id*="play" i]', '[role="button"]', 'button']) {
    for (const el of document.querySelectorAll(sel)) {
      if (!shown(el) || off(el)) continue;
      const s = sel === 'video' ? 1 : score(mark(el));
      if (!s || (best && best.s >= s)) continue;
      const r = el.getBoundingClientRect();
      best = {click: {x: r.left + r.width / 2, y: r.top + r.height / 2}, s};
    }
  }
  return best;
})()"#;

/// 有些站点把清单地址放在加密接口里：页面自己的 JS 解密后才交给播放器，浏览器里
/// 因此抓不到任何媒体请求（播放器甚至可能根本不创建）。这里不逆向它的算法，只包住
/// 页面解密结果的出口（decodeURIComponent / TextDecoder.decode / JSON.parse），把
/// 流经的明文清单地址收进 window.__pcMedia，再由解析器去验证。
const MEDIA_HOOK_JS: &str = r#"(() => {
  if (window.__pcMedia) return;
  const found = [];
  window.__pcMedia = found;
  const keep = text => {
    if (typeof text !== 'string' || text.length < 12 || text.length > 400000) return;
    if (!/\.m3u8|\.mpd/.test(text)) return;
    const plain = text.replace(/\\\//g, '/');
    const matches = plain.match(/https?:\/\/[^"'\s\\<>]+\.(?:m3u8|mpd)[^"'\s\\<>]*/g);
    if (!matches) return;
    for (const url of matches) if (!found.includes(url) && found.length < 20) found.push(url);
  };
  const decode = window.decodeURIComponent;
  window.decodeURIComponent = function (text) { const out = decode.call(window, text); keep(out); return out; };
  const parse = JSON.parse;
  JSON.parse = function (text, ...rest) { const out = parse.call(this, text, ...rest); keep(text); return out; };
  if (window.TextDecoder) {
    const decodeBytes = TextDecoder.prototype.decode;
    TextDecoder.prototype.decode = function (...args) { const out = decodeBytes.apply(this, args); keep(out); return out; };
  }
})();"#;

/// 打开浏览器和连接调试端口大约要这么久，每次尝试都要把它算进去。
const BROWSER_STARTUP: Duration = Duration::from_secs(20);
/// 少于这么久就没必要再开一次浏览器。
const MIN_ATTEMPT: Duration = Duration::from_secs(10);

/// 每次尝试能用多少时间：给浏览器启动和收尾留出余量，小预算时按比例缩小，免得
/// 两次尝试连启动开销一起超出预算——那样外层总时限会先到，把已经查到的真实原因
/// 掐成一句“Parsing exceeded … seconds”。
fn attempt_within(remaining: Duration, mobile: bool) -> Option<Duration> {
    let slack = BROWSER_STARTUP.min(remaining / 6);
    let usable = remaining.saturating_sub(slack);
    let share = if mobile { usable } else { usable / 2 };
    (share >= MIN_ATTEMPT).then_some(share)
}

/// 先按桌面身份解析，页面没有给出媒体请求时再用手机身份重试一次：只给
/// 桌面浏览器跳搜索引擎的站点必须走这条路。两次尝试共用调用方给的时间
/// 预算，避免第一次把全部时间用掉。
pub async fn capture(
    config: &Config,
    page: &str,
    headers: &BTreeMap<String, String>,
    budget: Duration,
) -> Result<Vec<(Url, BTreeMap<String, String>)>> {
    executable(config)
        .context("Chrome/Chromium not found. Install a browser or set PC_BROWSER.")?;
    // 留一点余量给收尾和上报：两次尝试用满预算时，外层总时限不该抢先掐断。
    let budget = budget.saturating_sub(Duration::from_secs(10));
    let started = tokio::time::Instant::now();
    let mut reasons = Vec::new();
    for mobile in [false, true] {
        let Some(within) = attempt_within(budget.saturating_sub(started.elapsed()), mobile) else {
            break;
        };
        let label = if mobile { "Mobile" } else { "Desktop" };
        match capture_once(config, page, headers, mobile, within).await {
            Ok(CaptureResult::Media(found)) => return Ok(found),
            Ok(CaptureResult::MobileOnly) => reasons.push(format!(
                "{label}: the page asks for a mobile browser, but no playlist appeared"
            )),
            Ok(CaptureResult::NoMedia {
                reason,
                attempted_media,
            }) => {
                reasons.push(format!("{label}: {reason}"));
                // 桌面这次已经在播放器页上取过流，再换手机身份没有意义。
                if !mobile && attempted_media {
                    break;
                }
            }
            Err(e) => reasons.push(format!("{label}: {e:#}")),
        }
    }
    anyhow::bail!(
        "{} Login or a playback action may be required.",
        if reasons.is_empty() {
            "Not enough time remained to parse the page.".to_owned()
        } else {
            reasons.join(" ")
        }
    )
}

async fn capture_once(
    config: &Config,
    page: &str,
    headers: &BTreeMap<String, String>,
    mobile: bool,
    within: Duration,
) -> Result<CaptureResult> {
    let exe = executable(config)
        .context("Chrome/Chromium not found. Install a browser or set PC_BROWSER.")?;
    let profile = tempfile::tempdir()?;
    let mut cmd = Command::new(exe);
    cmd.args([
        "--headless=new",
        "--remote-debugging-port=0",
        "--remote-allow-origins=*",
        "--no-first-run",
        "--no-default-browser-check",
        "--autoplay-policy=no-user-gesture-required",
    ])
    .arg(format!("--user-data-dir={}", profile.path().display()))
    .arg("about:blank")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .kill_on_drop(true);
    if mobile {
        cmd.arg(format!("--user-agent={MOBILE_UA}"))
            .arg("--window-size=390,844");
    }
    if config.browser_no_sandbox {
        cmd.arg("--no-sandbox");
    }
    let mut child = cmd.spawn().context("Could not start Chromium")?;
    // 会话自己按 within 收敛；这层只兜住浏览器连不上之类的不返回。
    let hard = within + BROWSER_STARTUP;
    let result = tokio::time::timeout(
        hard,
        capture_session(profile.path(), &mut child, page, headers, mobile, within),
    )
    .await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    match result {
        Ok(result) => result,
        Err(_) => anyhow::bail!("browser session exceeded {} seconds", hard.as_secs()),
    }
}

async fn send<S>(
    ws: &mut WebSocketStream<S>,
    id: i64,
    session: Option<&str>,
    method: &str,
    params: Value,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut message = json!({"id": id, "method": method, "params": params});
    if let Some(session) = session {
        message["sessionId"] = json!(session);
    }
    ws.send(Message::Text(message.to_string().into())).await?;
    Ok(())
}

async fn await_result<S>(
    ws: &mut WebSocketStream<S>,
    id: i64,
    session: Option<&str>,
) -> Result<Value>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let m = ws.next().await.context("Browser connection closed")??;
        let text = m.to_text().unwrap_or("");
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        if v["id"].as_i64() == Some(id)
            && session.is_none_or(|s| v["sessionId"].as_str() == Some(s))
        {
            return Ok(v);
        }
    }
}

async fn capture_session(
    profile: &std::path::Path,
    child: &mut tokio::process::Child,
    page: &str,
    headers: &BTreeMap<String, String>,
    mobile: bool,
    within: Duration,
) -> Result<CaptureResult> {
    let mut deadline = tokio::time::Instant::now() + within;
    let portfile = profile.join("DevToolsActivePort");
    let info = loop {
        if let Ok(s) = tokio::fs::read_to_string(&portfile).await {
            break s;
        }
        if let Some(exit) = child.try_wait()? {
            anyhow::bail!("Browser exited early: {exit}");
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Browser did not open its debug port in time");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let mut lines = info.lines();
    let port = lines.next().context("Missing debug port")?;
    let path = lines.next().context("Missing debug address")?;
    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}{path}")).await?;

    // 浏览器连接和每个帧会话共用这一条连接，只有全局递增的 id 才不会串响应。
    let mut counter = 0i64;
    let mut next_id = move || {
        counter += 1;
        counter
    };

    let create_id = next_id();
    send(
        &mut ws,
        create_id,
        None,
        "Target.createTarget",
        json!({"url":"about:blank"}),
    )
    .await?;
    let created = await_result(&mut ws, create_id, None).await?;
    let target = created["result"]["targetId"]
        .as_str()
        .context("Could not create a browser tab")?
        .to_owned();
    let attach_id = next_id();
    send(
        &mut ws,
        attach_id,
        None,
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
    )
    .await?;
    let attached = await_result(&mut ws, attach_id, None).await?;
    let session = attached["result"]["sessionId"]
        .as_str()
        .context("Could not attach to the browser tab")?
        .to_owned();

    send(
        &mut ws,
        next_id(),
        Some(&session),
        "Network.enable",
        json!({}),
    )
    .await?;
    send(
        &mut ws,
        next_id(),
        Some(&session),
        "Network.setExtraHTTPHeaders",
        json!({"headers":headers}),
    )
    .await?;
    send(&mut ws, next_id(), Some(&session), "Page.enable", json!({})).await?;
    // 注入要在页面脚本之前完成，所以放在导航之前。
    send(
        &mut ws,
        next_id(),
        Some(&session),
        "Page.addScriptToEvaluateOnNewDocument",
        json!({"source":MEDIA_HOOK_JS}),
    )
    .await?;
    // 播放器常放在跨站 iframe 里，那是独立的浏览器会话；不接管它就只能看到主页面。
    send(
        &mut ws,
        next_id(),
        Some(&session),
        "Target.setAutoAttach",
        json!({"autoAttach":true,"waitForDebuggerOnStart":true,"flatten":true}),
    )
    .await?;
    if mobile {
        send(
            &mut ws,
            next_id(),
            Some(&session),
            "Emulation.setUserAgentOverride",
            json!({"userAgent": MOBILE_UA, "platform": "Android"}),
        )
        .await?;
        send(
            &mut ws,
            next_id(),
            Some(&session),
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":2,"mobile":true}),
        )
        .await?;
        send(
            &mut ws,
            next_id(),
            Some(&session),
            "Emulation.setTouchEmulationEnabled",
            json!({"enabled":true,"maxTouchPoints":5}),
        )
        .await?;
    }
    send(
        &mut ws,
        next_id(),
        Some(&session),
        "Page.navigate",
        json!({"url":page}),
    )
    .await?;

    let mut requests = HashMap::new();
    let mut candidates = HashMap::new();
    let mut answered = HashSet::new();
    let mut failures = Vec::new();
    let mut sessions = vec![session.clone()];
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut mobile_check_ids = HashSet::new();
    let mut mobile_only = false;
    let mut landed = String::new();
    let mut blocked_by = None;
    let mut probe_id = None;
    let mut media_probe_id = None;
    let mut probes = 0;
    let mut probe_due = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            _=tokio::time::sleep_until(deadline)=>break,
            _=tick.tick()=>{
                for target in sessions.clone() {
                    send(&mut ws,next_id(),Some(&target),"Runtime.evaluate",json!({"expression":"document.querySelectorAll('video').forEach(v=>{v.muted=true;v.play().catch(()=>{})})"})).await?;
                }
                if !mobile && found.is_empty() {
                    let check_id=next_id();
                    mobile_check_ids.insert(check_id);
                    send(&mut ws,check_id,Some(&session),"Runtime.evaluate",json!({"expression":"(() => { const t = document.body?.innerText || ''; const meta = document.querySelector('meta[name=\\\"applicable-device\\\"]')?.content || ''; return (t.length < 600 && (t.includes('本站仅支持移动设备访问') || t.includes('only supports mobile device access'))) || /(^|[,;\\s])mobile([,;\\s]|$)/i.test(meta); })()","returnByValue":true})).await?;
                }
                if found.is_empty() && probes < 3 && tokio::time::Instant::now() >= probe_due {
                    probes += 1;
                    probe_due = tokio::time::Instant::now() + Duration::from_secs(8);
                    let id=next_id();
                    probe_id=Some(id);
                    send(&mut ws,id,Some(&session),"Runtime.evaluate",json!({"expression":PLAY_ENTRY_JS,"returnByValue":true})).await?;
                }
                if found.len() < 4 {
                    let id=next_id();
                    media_probe_id=Some(id);
                    send(&mut ws,id,Some(&session),"Runtime.evaluate",json!({"expression":"JSON.stringify(window.__pcMedia||[])","returnByValue":true})).await?;
                }
            }
            m=ws.next()=>{
                let Some(m)=m else {break;};let m=m?;let Ok(v)=serde_json::from_str::<Value>(m.to_text().unwrap_or("")) else {continue;};
                let from=v["sessionId"].as_str().unwrap_or("").to_owned();
                if v["id"].as_i64().is_some_and(|id| mobile_check_ids.remove(&id)) && v["result"]["result"]["value"]==true {
                    mobile_only=true;
                    break;
                }
                if v["id"].as_i64()==probe_id {
                    probe_id=None;
                    let plan=v["result"]["result"]["value"].clone();
                    if let Some(entry)=plan["navigate"].as_str() {
                        send(&mut ws,next_id(),Some(&session),"Page.navigate",json!({"url":entry})).await?;
                    } else if let (Some(x),Some(y))=(plan["click"]["x"].as_f64(),plan["click"]["y"].as_f64()) {
                        for kind in ["mousePressed","mouseReleased"] {
                            send(&mut ws,next_id(),Some(&session),"Input.dispatchMouseEvent",json!({"type":kind,"x":x,"y":y,"button":"left","clickCount":1})).await?;
                        }
                    }
                }
                if v["id"].as_i64()==media_probe_id {
                    media_probe_id=None;
                    if let Some(list)=v["result"]["result"]["value"].as_str()
                        && let Ok(items)=serde_json::from_str::<Vec<String>>(list) {
                        for raw in items {
                            if !seen.insert(raw.clone()) {continue;}
                            if let Ok(u)=Url::parse(&raw) {
                                let mut h=headers.clone();
                                h.entry("referer".into()).or_insert(page.into());
                                if mobile { h.insert("user-agent".into(), MOBILE_UA.into()); }
                                if found.is_empty(){deadline=tokio::time::Instant::now()+Duration::from_secs(2);}
                                found.push((u,h));
                            }
                        }
                        if found.len()>=4 {break;}
                    }
                }
                match v["method"].as_str().unwrap_or("") {
                    "Target.attachedToTarget"=>{
                        let child=v["params"]["sessionId"].as_str().unwrap_or("").to_owned();
                        if child.is_empty() {continue;}
                        let kind=v["params"]["targetInfo"]["type"].as_str().unwrap_or("");
                        if kind=="iframe"||kind=="page" {
                            send(&mut ws,next_id(),Some(&child),"Network.enable",json!({})).await?;
                            send(&mut ws,next_id(),Some(&child),"Network.setExtraHTTPHeaders",json!({"headers":headers})).await?;
                            send(&mut ws,next_id(),Some(&child),"Page.enable",json!({})).await?;
                            send(&mut ws,next_id(),Some(&child),"Target.setAutoAttach",json!({"autoAttach":true,"waitForDebuggerOnStart":true,"flatten":true})).await?;
                        }
                        if !sessions.contains(&child) {sessions.push(child.clone());}
                        if v["params"]["waitingForDebugger"].as_bool()==Some(true) {
                            send(&mut ws,next_id(),Some(&child),"Runtime.runIfWaitingForDebugger",json!({})).await?;
                        }
                    }
                    "Page.frameNavigated"=>{
                        if from==session && v["params"]["frame"]["parentId"].is_null() {
                            landed=v["params"]["frame"]["url"].as_str().unwrap_or("").to_owned();
                            if let Ok(u)=Url::parse(&landed)
                                && !mobile
                                && u.host_str().is_some_and(is_search_engine) {
                                blocked_by=u.host_str().map(str::to_owned);
                                break;
                            }
                        }
                    }
                    "Network.requestWillBeSent"=>{
                        let key=format!("{from}:{}",v["params"]["requestId"].as_str().unwrap_or(""));
                        let url=v["params"]["request"]["url"].as_str().unwrap_or("").to_owned();
                        if looks_like_media(&url,"") {candidates.insert(key.clone(),url);}
                        requests.insert(key,v["params"]["request"].clone());
                    }
                    "Network.responseReceived"=>{
                        let key=format!("{from}:{}",v["params"]["requestId"].as_str().unwrap_or(""));
                        answered.insert(key.clone());
                        let r=&v["params"]["response"];
                        let mime=r["mimeType"].as_str().unwrap_or("").to_lowercase();
                        let raw=r["url"].as_str().unwrap_or("");
                        if looks_like_media(raw,&mime) && seen.insert(raw.to_owned())
                            && let Ok(u)=Url::parse(raw) {
                            let mut h=headers.clone();
                            h.entry("referer".into()).or_insert(page.into());
                            if mobile { h.insert("user-agent".into(), MOBILE_UA.into()); }
                            if let Some(req)=requests.get(&key)&& let Some(map)=req["headers"].as_object(){
                                for(k,val)in map{if ["cookie","referer","user-agent","authorization","origin"].contains(&k.to_lowercase().as_str())&& let Some(val)=val.as_str(){h.insert(k.to_lowercase(),val.into());}}
                            }
                            if found.is_empty(){deadline=tokio::time::Instant::now()+Duration::from_secs(2);}
                            found.push((u,h)); if found.len()>=4{break;}
                        }
                    }
                    "Network.loadingFailed"=>{
                        let key=format!("{from}:{}",v["params"]["requestId"].as_str().unwrap_or(""));
                        if let Some(url)=candidates.get(&key) {
                            failures.push(format!("{url} ({})",v["params"]["errorText"].as_str().unwrap_or("failed")));
                        }
                    }
                    _=>{}
                }
                if !found.is_empty() && v["method"].as_str()==Some("Page.loadEventFired") {break;}
            }
        }
    }
    if mobile_only && found.is_empty() {
        return Ok(CaptureResult::MobileOnly);
    }
    if found.is_empty() {
        // 区分三种情况：页面没发起请求、请求失败、请求发出但没有响应。
        let mut reason = String::from("No media requests detected after loading the page.");
        if let Some(host) = &blocked_by {
            reason.push_str(&format!(
                " The desktop session was redirected to {host} instead of a player page."
            ));
        } else if !landed.is_empty() && landed != page {
            reason.push_str(&format!(" The page ended at {landed}."));
        }
        if probes > 0 {
            reason.push_str(&format!(" A play control was used {probes} time(s)."));
        }
        if !failures.is_empty() {
            reason.push_str(&format!(
                " Media candidates failed: {}.",
                failures
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        } else {
            let pending = candidates
                .iter()
                .filter(|(key, _)| !answered.contains(*key))
                .map(|(_, url)| url.as_str())
                .take(2)
                .collect::<Vec<_>>();
            if !pending.is_empty() {
                reason.push_str(&format!(
                    " Media candidates got no response: {}.",
                    pending.join(", ")
                ));
            }
        }
        return Ok(CaptureResult::NoMedia {
            reason,
            attempted_media: !candidates.is_empty(),
        });
    }
    // Cookie 可能不出现在 requestWillBeSent 中，显式读取当前媒体域的 Cookie。
    let cookie_id = next_id();
    send(
        &mut ws,
        cookie_id,
        Some(&session),
        "Network.getCookies",
        json!({"urls":found.iter().map(|(u,_)|u.as_str()).collect::<Vec<_>>()}),
    )
    .await?;
    let cookies = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let m = ws.next().await.context("Browser connection closed")??;
            let text = m.to_text().unwrap_or("");
            let Ok(v) = serde_json::from_str::<Value>(text) else {
                continue;
            };
            if v["id"].as_i64() == Some(cookie_id) && v["sessionId"].as_str() == Some(&session) {
                return Ok::<Value, anyhow::Error>(v);
            }
        }
    })
    .await;
    if let Ok(Ok(v)) = cookies
        && let Some(items) = v["result"]["cookies"].as_array()
    {
        for (u, h) in &mut found {
            let values = items
                .iter()
                .filter(|c| {
                    let domain = c["domain"].as_str().unwrap_or("").trim_start_matches('.');
                    let host = u.host_str().unwrap_or("");
                    (host == domain || host.ends_with(&format!(".{domain}")))
                        && u.path().starts_with(c["path"].as_str().unwrap_or("/"))
                })
                .filter_map(|c| Some(format!("{}={}", c["name"].as_str()?, c["value"].as_str()?)))
                .collect::<Vec<_>>();
            if !values.is_empty() {
                h.insert("cookie".into(), values.join("; "));
            }
        }
    }
    Ok(CaptureResult::Media(found))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, response::Html, routing::get};
    use clap::Parser;

    async fn test_page(
        html: String,
        playlist: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/page", get(move || async move { Html(html) }))
            .route(
                "/stream.m3u8",
                get(move || async move {
                    (
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/vnd.apple.mpegurl",
                        )],
                        playlist,
                    )
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (address, server)
    }

    fn browser_config() -> Option<Config> {
        let mut config = Config::parse_from(["pagecatch"]);
        config.browser_no_sandbox = cfg!(target_os = "linux");
        executable(&config).map(|_| config)
    }

    #[test]
    fn both_browser_attempts_fit_the_budget() {
        for budget in [
            Duration::from_secs(250),
            Duration::from_secs(60),
            Duration::from_secs(40),
        ] {
            let desktop = attempt_within(budget, false).expect("预算够时必须开桌面尝试");
            // 桌面用掉自己的份额（+约 3 秒启动）后，手机重试仍要开得起来。
            let mobile = attempt_within(budget - desktop - Duration::from_secs(3), true)
                .expect("预算够时手机重试必须开得起来");
            assert!(
                desktop + mobile + Duration::from_secs(6) <= budget,
                "{budget:?} 装不下两次尝试，真实原因会被外层总时限掐掉"
            );
        }
        // 余量太少就不再开新的一次：桌面分不到，手机也分不到。
        assert_eq!(attempt_within(Duration::from_secs(20), false), None);
        assert_eq!(attempt_within(Duration::from_secs(10), true), None);
    }

    #[tokio::test]
    async fn retries_a_page_that_explicitly_requires_mobile() {
        let Some(config) = browser_config() else {
            return;
        };
        let (address, server) = test_page(
            r#"<meta name="viewport" content="width=device-width"><body><script>
                if (navigator.userAgent.includes('Mobile') && navigator.maxTouchPoints > 0 && matchMedia('(max-width: 500px)').matches) {
                    document.body.innerText = 'Ready';
                    fetch('/stream.m3u8');
                } else {
                    document.body.innerText = '本站仅支持移动设备访问';
                }
            </script></body>"#
                .to_string(),
            PLAYLIST,
        )
        .await;
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(60),
        )
        .await;
        server.abort();
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.as_str(), format!("{address}/stream.m3u8"));
    }

    #[tokio::test]
    async fn waits_for_a_playlist_after_the_old_twenty_second_limit() {
        let Some(config) = browser_config() else {
            return;
        };
        let (address, server) = test_page(
            "<script>setTimeout(() => fetch('/stream.m3u8'), 23000)</script>".to_string(),
            PLAYLIST,
        )
        .await;
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(60),
        )
        .await;
        server.abort();
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.as_str(), format!("{address}/stream.m3u8"));
    }

    /// 播放器放在跨站 iframe 里时，媒体请求属于另一个浏览器会话。
    #[tokio::test]
    async fn finds_a_playlist_requested_from_a_cross_origin_iframe() {
        let Some(config) = browser_config() else {
            return;
        };
        let (frame_address, frame_server) = test_page(
            "<script>fetch('/stream.m3u8')</script>".to_string(),
            PLAYLIST,
        )
        .await;
        // 换个主机名，浏览器才会把它当成跨站 iframe 单独托管。
        let frame_page = format!("{}/page", frame_address.replace("127.0.0.1", "localhost"));
        let (address, server) = test_page(
            format!(r#"<iframe src="{frame_page}" width="320" height="180"></iframe>"#),
            PLAYLIST,
        )
        .await;
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(60),
        )
        .await;
        frame_server.abort();
        server.abort();
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].0.as_str(),
            frame_page.replace("/page", "/stream.m3u8")
        );
    }

    /// 详情页只放一个站内播放入口，播放器要进去才创建。
    #[tokio::test]
    async fn follows_the_page_own_play_entry() {
        let Some(config) = browser_config() else {
            return;
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route(
                "/page",
                get(|| async {
                    Html(r#"<a href="/play"><div class="btn-play">立即播放</div></a>"#)
                }),
            )
            .route(
                "/play",
                get(|| async { Html("<script>fetch('/stream.m3u8')</script>") }),
            )
            .route(
                "/stream.m3u8",
                get(|| async {
                    (
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/vnd.apple.mpegurl",
                        )],
                        PLAYLIST,
                    )
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(40),
        )
        .await;
        server.abort();
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.as_str(), format!("{address}/stream.m3u8"));
    }

    /// 清单地址只经过页面自己的解密、页面从未请求它时，也要能拿到。
    #[tokio::test]
    async fn catches_a_playlist_that_only_the_page_decrypts() {
        let Some(config) = browser_config() else {
            return;
        };
        let (address, server) = test_page(
            r#"<script>
                const payload = JSON.stringify({data:{video:{url: location.origin + '/stream.m3u8'}}});
                const decrypted = decodeURIComponent(payload);
                document.title = JSON.parse(decrypted).data.video.url;
            </script>"#
                .to_string(),
            PLAYLIST,
        )
        .await;
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(40),
        )
        .await;
        server.abort();
        let found = result.unwrap();
        assert_eq!(found[0].0.as_str(), format!("{address}/stream.m3u8"));
    }

    /// 站点只给桌面浏览器跳搜索引擎，播放页只有手机身份才拿得到。
    #[tokio::test]
    async fn uses_a_mobile_browser_when_desktop_visitors_are_sent_away() {
        let Some(config) = browser_config() else {
            return;
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route(
                "/page",
                get(|| async {
                    Html(
                        r#"<script>
                            if (navigator.userAgent.includes('Mobile')) {
                                fetch('/stream.m3u8');
                            } else {
                                window.location.href = '/away';
                            }
                        </script>"#,
                    )
                }),
            )
            .route(
                "/away",
                get(|| async { Html("<p>Search somewhere else.</p>") }),
            )
            .route(
                "/stream.m3u8",
                get(|| async {
                    (
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/vnd.apple.mpegurl",
                        )],
                        PLAYLIST,
                    )
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = capture(
            &config,
            &format!("{address}/page"),
            &BTreeMap::new(),
            Duration::from_secs(40),
        )
        .await;
        server.abort();
        let found = result.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.as_str(), format!("{address}/stream.m3u8"));
    }
}
