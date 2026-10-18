//! Update check (0.2.3.5): asks GitHub for the latest published release and
//! compares its tag with the running version. Runs on a background thread with
//! a short timeout; nothing is downloaded or opened unless the user clicks.
use std::sync::mpsc;
use std::time::Duration;

/// GitHub "latest release" endpoint (drafts and prereleases are never "latest",
/// and we double-check the flags anyway). The `CHIPWAVE_UPDATE_URL` environment
/// variable overrides it (for testing against a mock http server).
pub const LATEST_URL: &str =
    "https://api.github.com/repos/postmanpat-303/ChipWaveTracker/releases/latest";
/// shown when no release page is known
pub const RELEASES_PAGE: &str = "https://github.com/postmanpat-303/ChipWaveTracker/releases";
/// startup checks run at most this often (seconds)
pub const STARTUP_INTERVAL: u64 = 24 * 60 * 60;
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// tag as published, e.g. "v0.2.3.5"
    pub tag: String,
    /// display version, tag without the leading "v"
    pub version: String,
    pub name: String,
    /// release notes (GitHub markdown)
    pub body: String,
    /// release page
    pub page: String,
    /// the installer asset for this OS (Windows setup .exe; macOS .dmg), if the release has one
    pub installer: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
}

impl Release {
    /// where "Download installer" goes: the installer asset, else the release page
    pub fn download_url(&self) -> &str {
        self.installer.as_deref().unwrap_or(&self.page)
    }
}

/// "v0.2.3.4" / "0.2.3" -> [0, 2, 3, 4] / [0, 2, 3]. Anything after a '-' or '+'
/// (e.g. "-beta") is ignored; missing trailing parts compare as 0.
pub fn parse_version(s: &str) -> Option<Vec<u32>> {
    let s = s.trim();
    let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let core = s.split(['-', '+', ' ']).next()?;
    if core.is_empty() {
        return None;
    }
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// true when `latest` is a newer version than `current` (3- and 4-part versions)
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) => {
            let n = a.len().max(b.len());
            let pad = |v: &[u32]| {
                (0..n)
                    .map(|i| v.get(i).copied().unwrap_or(0))
                    .collect::<Vec<_>>()
            };
            pad(&a) > pad(&b)
        }
        _ => false,
    }
}

/// parse the GitHub releases API JSON (one release object)
pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("unexpected reply from GitHub ({e})"))?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let tag = s("tag_name");
    if tag.is_empty() || parse_version(&tag).is_none() {
        return Err(format!("unexpected release tag \"{tag}\""));
    }
    let assets = v.get("assets").and_then(|a| a.as_array());
    let url_of = |pred: &dyn Fn(&str) -> bool| {
        assets.and_then(|a| {
            a.iter().find_map(|x| {
                let name = x.get("name")?.as_str()?.to_ascii_lowercase();
                let url = x.get("browser_download_url")?.as_str()?;
                pred(&name).then(|| url.to_string())
            })
        })
    };
    // macOS: the universal .dmg; elsewhere: the Windows setup .exe
    let installer = if cfg!(target_os = "macos") {
        url_of(&|n| n.ends_with(".dmg") && n.contains("macos"))
            .or_else(|| url_of(&|n| n.ends_with(".dmg")))
    } else {
        url_of(&|n| n.ends_with(".exe") && n.contains("setup"))
            .or_else(|| url_of(&|n| n.ends_with(".exe")))
    };
    let page = s("html_url");
    Ok(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag,
        name: s("name"),
        body: s("body"),
        page: if page.is_empty() {
            RELEASES_PAGE.into()
        } else {
            page
        },
        installer,
        draft: v.get("draft").and_then(|x| x.as_bool()).unwrap_or(false),
        prerelease: v
            .get("prerelease")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
    })
}

/// what a check found
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Newer(Release),
    /// up to date; the latest published version
    Latest(String),
    Failed(String),
}

/// compare a fetched release with the running version
pub fn evaluate(fetched: Result<Release, String>, current: &str) -> Outcome {
    match fetched {
        Err(e) => Outcome::Failed(e),
        Ok(r) if r.draft || r.prerelease => Outcome::Latest(current.to_string()),
        Ok(r) if is_newer(&r.version, current) => Outcome::Newer(r),
        Ok(r) => Outcome::Latest(r.version),
    }
}

fn url() -> String {
    std::env::var("CHIPWAVE_UPDATE_URL").unwrap_or_else(|_| LATEST_URL.into())
}

/// blocking fetch (call from a background thread); 5 s overall timeout
pub fn fetch_latest() -> Result<Release, String> {
    fetch_from(&url())
}

/// blocking fetch of one release JSON from `url`
pub fn fetch_from(url: &str) -> Result<Release, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(format!("ChipWave-Tracker/{}", crate::VERSION))
        .build()
        .into();
    let mut resp = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(404) => "no published release was found".to_string(),
            ureq::Error::StatusCode(403) | ureq::Error::StatusCode(429) => {
                "GitHub is rate-limiting requests right now; try again later".to_string()
            }
            ureq::Error::StatusCode(c) => format!("GitHub answered with HTTP {c}"),
            ureq::Error::Timeout(_) => "the server did not answer in time".to_string(),
            e => format!("no connection ({e})"),
        })?;
    let body = resp
        .body_mut()
        .with_config()
        .limit(4 << 20)
        .read_to_string()
        .map_err(|e| format!("download failed ({e})"))?;
    parse_release(&body)
}

/// start a check on a background thread; the result arrives on the receiver
#[cfg_attr(test, allow(dead_code))] // UI tests deliver results through a mock channel
pub fn spawn_check() -> mpsc::Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            let _ = tx.send(evaluate(fetch_latest(), crate::VERSION));
        })
        .ok();
    rx
}

/// seconds since the Unix epoch
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// is a quiet startup check due? (enabled and the last one was >= 24 h ago;
/// a clock that went backwards also counts as due)
pub fn startup_due(enabled: bool, last: u64, now: u64) -> bool {
    enabled && (now < last || now - last >= STARTUP_INTERVAL)
}

/// one line of simply rendered release notes
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Heading(String),
    Bullet(String),
    Text(String),
    Blank,
}

/// very small markdown renderer: headings, bullets, text; strips **, __, `
/// and turns [label](url) into "label"
pub fn notes_lines(md: &str) -> Vec<Line> {
    let clean = |s: &str| {
        let mut out = String::new();
        let mut rest = s;
        // [label](url) -> label
        while let Some(i) = rest.find('[') {
            let tail = &rest[i..];
            match (tail.find("]("), tail.find(')')) {
                (Some(j), Some(k)) if j < k => {
                    out.push_str(&rest[..i]);
                    out.push_str(&tail[1..j]);
                    rest = &tail[k + 1..];
                }
                _ => {
                    out.push_str(&rest[..=i]);
                    rest = &rest[i + 1..];
                }
            }
        }
        out.push_str(rest);
        out.replace("**", "").replace("__", "").replace('`', "")
    };
    let mut lines = vec![];
    for raw in md.replace("\r\n", "\n").lines() {
        let t = raw.trim();
        let l = if t.is_empty() {
            if matches!(lines.last(), Some(Line::Blank) | None) {
                continue;
            }
            Line::Blank
        } else if t.starts_with('#') {
            Line::Heading(clean(t.trim_start_matches('#').trim()))
        } else if let Some(b) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            Line::Bullet(clean(b))
        } else {
            Line::Text(clean(t))
        };
        lines.push(l);
    }
    while matches!(lines.last(), Some(Line::Blank)) {
        lines.pop();
    }
    lines
}

/// the installer `SAMPLE_JSON` offers on this OS
#[cfg(test)]
pub(crate) const SAMPLE_INSTALLER: &str = if cfg!(target_os = "macos") {
    "/ChipWave-0.2.3.9-macos-universal.dmg"
} else {
    "/ChipWave-0.2.3.9-setup-x64.exe"
};

#[cfg(test)]
pub(crate) const SAMPLE_JSON: &str = r###"{
  "url": "https://api.github.com/repos/postmanpat-303/ChipWaveTracker/releases/1",
  "html_url": "https://github.com/postmanpat-303/ChipWaveTracker/releases/tag/v0.2.3.9",
  "tag_name": "v0.2.3.9",
  "name": "ChipWave Tracker 0.2.3.9",
  "draft": false,
  "prerelease": false,
  "body": "## What's new\r\n\r\n- **Update check** in Help\r\n- See [docs](https://example.com/docs)\r\n\r\nThanks!",
  "assets": [
    {"name": "ChipWave-0.2.3.9-windows-x64.zip",
     "browser_download_url": "https://github.com/postmanpat-303/ChipWaveTracker/releases/download/v0.2.3.9/ChipWave-0.2.3.9-windows-x64.zip"},
    {"name": "ChipWave-0.2.3.9-setup-x64.exe",
     "browser_download_url": "https://github.com/postmanpat-303/ChipWaveTracker/releases/download/v0.2.3.9/ChipWave-0.2.3.9-setup-x64.exe"},
    {"name": "ChipWave-0.2.3.9-macos-universal.dmg",
     "browser_download_url": "https://github.com/postmanpat-303/ChipWaveTracker/releases/download/v0.2.3.9/ChipWave-0.2.3.9-macos-universal.dmg"},
    {"name": "demo-0.2.3.9.mp3",
     "browser_download_url": "https://github.com/postmanpat-303/ChipWaveTracker/releases/download/v0.2.3.9/demo-0.2.3.9.mp3"}
  ]
}"###;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_with_three_and_four_parts() {
        assert!(is_newer("0.2.3.4", "0.2.3"));
        assert!(is_newer("0.2.4", "0.2.3.4"));
        assert!(is_newer("v0.2.3.5", "0.2.3.4"));
        assert!(is_newer("V1.0", "0.9.9.9"));
        assert!(is_newer("0.2.10", "0.2.9"), "numeric, not text");
        assert!(!is_newer("0.2.3", "0.2.3.4"));
        assert!(!is_newer("0.2.3.4", "0.2.4"));
        assert!(!is_newer("v0.2.3.4", "0.2.3.4"));
        assert!(!is_newer("0.2.3", "0.2.3.0"), "missing parts are zero");
        assert!(!is_newer("0.2.3.0", "0.2.3"));
        assert!(!is_newer("nonsense", "0.2.3"));
        assert!(!is_newer("0.2.4", "garbage"));
        assert_eq!(parse_version("v0.2.3-beta"), Some(vec![0, 2, 3]));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v"), None);
        assert_eq!(parse_version("1..2"), None);
        // the running version itself parses
        assert!(parse_version(crate::VERSION).is_some());
    }

    #[test]
    fn parses_the_github_release_json() {
        let r = parse_release(SAMPLE_JSON).unwrap();
        assert_eq!(r.tag, "v0.2.3.9");
        assert_eq!(r.version, "0.2.3.9");
        assert_eq!(r.name, "ChipWave Tracker 0.2.3.9");
        assert!(r.page.ends_with("/releases/tag/v0.2.3.9"));
        assert!(r.download_url().ends_with(SAMPLE_INSTALLER));
        assert!(!r.draft && !r.prerelease);
        // no installer asset -> the download button goes to the release page
        let no_exe = SAMPLE_JSON
            .replace("setup-x64.exe", "setup-x64.msi")
            .replace("macos-universal.dmg", "macos-universal.pkg");
        let r = parse_release(&no_exe).unwrap();
        assert_eq!(r.installer, None);
        assert_eq!(r.download_url(), r.page);
        // broken / unexpected replies are errors, not panics
        assert!(parse_release("").is_err());
        assert!(parse_release("{\"message\":\"Not Found\"}").is_err());
        assert!(parse_release("[1,2]").is_err());
        assert!(parse_release(&SAMPLE_JSON.replace("v0.2.3.9", "latest")).is_err());
    }

    #[test]
    fn parses_the_real_published_v0_2_3_4_release() {
        // saved reply of the live API (releases/latest on 17 Oct 2026)
        let r = parse_release(include_str!("update_fixture_v0.2.3.4.json")).unwrap();
        assert_eq!(
            (r.tag.as_str(), r.version.as_str()),
            ("v0.2.3.4", "0.2.3.4")
        );
        if cfg!(target_os = "macos") {
            // that release had no Mac build: the button opens the release page
            assert_eq!(r.download_url(), r.page);
        } else {
            assert!(r
                .download_url()
                .ends_with("/ChipWave-0.2.3.4-setup-x64.exe"));
        }
        assert!(!notes_lines(&r.body).is_empty());
        assert!(is_newer("0.2.3.5", &r.version));
    }

    #[test]
    fn evaluate_reports_newer_latest_failed_and_ignores_prereleases() {
        let r = parse_release(SAMPLE_JSON).unwrap();
        assert!(matches!(
            evaluate(Ok(r.clone()), "0.2.3.4"),
            Outcome::Newer(_)
        ));
        assert_eq!(
            evaluate(Ok(r.clone()), "0.2.3.9"),
            Outcome::Latest("0.2.3.9".into())
        );
        assert_eq!(
            evaluate(Ok(r.clone()), "0.2.4"),
            Outcome::Latest("0.2.3.9".into())
        );
        let pre = Release {
            prerelease: true,
            ..r.clone()
        };
        assert_eq!(
            evaluate(Ok(pre), "0.2.3.4"),
            Outcome::Latest("0.2.3.4".into())
        );
        let draft = Release { draft: true, ..r };
        assert_eq!(
            evaluate(Ok(draft), "0.2.3.4"),
            Outcome::Latest("0.2.3.4".into())
        );
        assert_eq!(
            evaluate(Err("x".into()), "0.2.3.4"),
            Outcome::Failed("x".into())
        );
    }

    #[test]
    fn startup_check_at_most_once_a_day() {
        let day = STARTUP_INTERVAL;
        assert!(startup_due(true, 0, 1_000_000));
        assert!(!startup_due(true, 1_000_000, 1_000_000 + day - 1));
        assert!(startup_due(true, 1_000_000, 1_000_000 + day));
        assert!(
            startup_due(true, 1_000_000, 999_000),
            "clock went backwards"
        );
        assert!(!startup_due(false, 0, 1_000_000));
    }

    #[test]
    fn release_notes_render_simply() {
        let r = parse_release(SAMPLE_JSON).unwrap();
        assert_eq!(
            notes_lines(&r.body),
            [
                Line::Heading("What's new".into()),
                Line::Blank,
                Line::Bullet("Update check in Help".into()),
                Line::Bullet("See docs".into()),
                Line::Blank,
                Line::Text("Thanks!".into()),
            ]
        );
        assert_eq!(notes_lines(""), []);
        assert_eq!(notes_lines("a [b c"), [Line::Text("a [b c".into())]);
    }

    #[test]
    fn unreachable_server_fails_fast_without_panicking() {
        // nothing listens on port 9 of localhost: an immediate, friendly error
        let t = std::time::Instant::now();
        let e = fetch_from("http://127.0.0.1:9/latest").unwrap_err();
        assert!(t.elapsed() < Duration::from_secs(7), "{:?}", t.elapsed());
        assert!(!e.is_empty());
    }

    #[test]
    fn fetches_from_a_mock_http_server() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let srv = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = [0u8; 4096];
            let n = s.read(&mut req).unwrap();
            let req = String::from_utf8_lossy(&req[..n]).to_string();
            let body = SAMPLE_JSON;
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            req
        });
        let r = fetch_from(&format!("http://127.0.0.1:{port}/latest"));
        let req = srv.join().unwrap();
        assert_eq!(r.unwrap().version, "0.2.3.9");
        let lower = req.to_ascii_lowercase();
        assert!(lower.contains("user-agent: chipwave-tracker/"), "{req}");
        assert!(
            lower.contains("accept: application/vnd.github+json"),
            "{req}"
        );
    }
}
