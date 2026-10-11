use std::cell::OnceCell;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const API: &str = "https://api.github.com";
const PER_PAGE: usize = 100;
const MAX_PAGES: u32 = 50;
const ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub login: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub html_url: String,
    pub public_repos: u32,
    pub followers: u32,
    pub following: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: u32,
    pub forks_count: u32,
    pub fork: bool,
    pub archived: bool,
    #[serde(default)]
    pub private: bool,
    pub created_at: Option<String>,
    pub pushed_at: Option<String>,
}

pub struct Client {
    agent: ureq::Agent,
    token: Option<String>,
    me: OnceCell<Option<String>>,
}

impl Client {
    pub fn new(token: Option<String>) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(20)))
            .build();
        Self {
            agent: config.into(),
            token,
            me: OnceCell::new(),
        }
    }

    pub fn user(&self, login: &str) -> Result<User> {
        if !valid_login(login) {
            bail!("'{login}' is not a valid GitHub username");
        }
        self.get(&format!("/users/{login}")).map_err(|e| {
            match e.to_string().starts_with("not found") {
                true => anyhow!("no GitHub user or org called '{login}'"),
                false => e,
            }
        })
    }

    /// The user the token belongs to.
    pub fn me(&self) -> Result<String> {
        if self.token.is_none() {
            bail!("no username given and not logged in, pass a name or run `gh auth login`");
        }
        let user: User = self.get("/user")?;
        Ok(user.login)
    }

    fn is_me(&self, login: &str) -> bool {
        self.me
            .get_or_init(|| self.token.as_ref().and_then(|_| self.me().ok()))
            .as_deref()
            .is_some_and(|me| me.eq_ignore_ascii_case(login))
    }

    /// Private repos are included when the token can see them: your own
    /// account, or an org you're a member of.
    pub fn repos(&self, user: &User, with_private: bool) -> Result<Vec<Repo>> {
        let login = &user.login;
        let base = if user.kind == "Organization" {
            let kind = if with_private { "all" } else { "public" };
            format!("/orgs/{login}/repos?type={kind}")
        } else if with_private && self.is_me(login) {
            "/user/repos?affiliation=owner&visibility=all".to_string()
        } else {
            format!("/users/{login}/repos?type=owner")
        };
        paginate(|page| {
            self.get(&format!(
                "{base}&sort=pushed&per_page={PER_PAGE}&page={page}"
            ))
        })
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{API}{path}");
        let mut attempt = 1;
        let mut resp = loop {
            let mut req = self
                .agent
                .get(&url)
                .header("User-Agent", "gh-stats")
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28");
            if let Some(token) = &self.token {
                req = req.header("Authorization", format!("Bearer {token}"));
            }

            match req.call() {
                Ok(r) if !(500..600).contains(&r.status().as_u16()) || attempt == ATTEMPTS => {
                    break r
                }
                Err(e) if attempt == ATTEMPTS => {
                    return Err(anyhow!(e).context("could not reach api.github.com"))
                }
                _ => {}
            }
            sleep(Duration::from_millis(400 * 2u64.pow(attempt)));
            attempt += 1;
        };

        match resp.status().as_u16() {
            200 => {}
            401 => bail!("GitHub rejected the token, check GITHUB_TOKEN or run `gh auth login`"),
            403 | 429 if header(&resp, "x-ratelimit-remaining").as_deref() == Some("0") => {
                let reset = header(&resp, "x-ratelimit-reset").and_then(|v| v.parse().ok());
                bail!(
                    "{}",
                    rate_limit_message(reset, now_secs(), self.token.is_some())
                )
            }
            404 => bail!("not found: {path}"),
            code => bail!("GitHub returned {code} for {path}"),
        }

        resp.body_mut()
            .read_json()
            .with_context(|| format!("could not parse the response for {path}"))
    }
}

/// Token from GITHUB_TOKEN / GH_TOKEN, or from a logged in `gh` CLI.
pub fn find_token() -> Option<String> {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(token) = std::env::var(var) {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    let out = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let token = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!token.is_empty()).then_some(token)
}

pub fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && !login.ends_with('-')
        && login
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn paginate<T>(mut fetch: impl FnMut(u32) -> Result<Vec<T>>) -> Result<Vec<T>> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let batch = fetch(page)?;
        let last = batch.len() < PER_PAGE;
        all.extend(batch);
        if last {
            break;
        }
    }
    Ok(all)
}

fn rate_limit_message(reset: Option<u64>, now: u64, has_token: bool) -> String {
    let mut msg = String::from("hit the GitHub rate limit");
    if let Some(reset) = reset.filter(|r| *r > now) {
        msg += &format!(", it resets in {} min", (reset - now).div_ceil(60));
    }
    if !has_token {
        msg += ". Set GITHUB_TOKEN or run `gh auth login` to get 5000 requests an hour";
    }
    msg
}

fn header(resp: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    resp.headers().get(name)?.to_str().ok().map(str::to_string)
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_validation() {
        for ok in ["CanReader", "rust-lang", "a", "x1-y2", "jdoe_acme"] {
            assert!(valid_login(ok), "{ok}");
        }
        for bad in [
            "",
            "-abc",
            "abc-",
            "a/b",
            "a b",
            "../users",
            &"a".repeat(40),
        ] {
            assert!(!valid_login(bad), "{bad}");
        }
    }

    #[test]
    fn paginate_stops_on_short_page() {
        let mut calls = Vec::new();
        let all = paginate(|page| {
            calls.push(page);
            Ok(match page {
                1 | 2 => vec![0; PER_PAGE],
                _ => vec![0; 7],
            })
        })
        .unwrap();
        assert_eq!(all.len(), 2 * PER_PAGE + 7);
        assert_eq!(calls, [1, 2, 3]);
    }

    #[test]
    fn paginate_has_a_page_limit() {
        let all = paginate(|_| Ok(vec![0; PER_PAGE])).unwrap();
        assert_eq!(all.len(), MAX_PAGES as usize * PER_PAGE);
    }

    #[test]
    fn paginate_passes_errors_up() {
        let res: Result<Vec<u8>> = paginate(|_| bail!("boom"));
        assert!(res.is_err());
    }

    #[test]
    fn rate_limit_text() {
        let msg = rate_limit_message(Some(1_000 + 90), 1_000, false);
        assert!(msg.contains("resets in 2 min"));
        assert!(msg.contains("gh auth login"));

        let msg = rate_limit_message(Some(10), 1_000, true);
        assert_eq!(msg, "hit the GitHub rate limit");
    }
}
