use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

const API: &str = "https://api.github.com";
const PER_PAGE: usize = 100;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub login: String,
    pub name: Option<String>,
    pub public_repos: u32,
    pub followers: u32,
    pub following: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: u32,
    pub forks_count: u32,
    pub fork: bool,
    pub archived: bool,
}

pub struct Client {
    agent: ureq::Agent,
    token: Option<String>,
}

impl Client {
    pub fn new(token: Option<String>) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
            token,
        }
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let mut req = self
            .agent
            .get(format!("{API}{path}"))
            .header("User-Agent", "gh-stats-cli")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        let mut resp = req
            .call()
            .with_context(|| format!("request to {path} failed"))?;
        match resp.status().as_u16() {
            200 => {}
            404 => bail!("not found: {path}"),
            401 => bail!("bad token, check GITHUB_TOKEN"),
            403 | 429 if is_rate_limited(&resp) => {
                bail!("hit the GitHub rate limit, set GITHUB_TOKEN to get a higher limit")
            }
            code => bail!("GitHub returned {code} for {path}"),
        }

        resp.body_mut()
            .read_json()
            .with_context(|| format!("could not parse response from {path}"))
    }

    pub fn user(&self, login: &str) -> Result<User> {
        self.get(&format!("/users/{login}"))
            .with_context(|| format!("could not load user {login}"))
    }

    pub fn repos(&self, login: &str) -> Result<Vec<Repo>> {
        let mut all = Vec::new();
        let mut page = 1;
        loop {
            let batch: Vec<Repo> = self.get(&format!(
                "/users/{login}/repos?per_page={PER_PAGE}&page={page}&type=owner"
            ))?;
            let done = batch.len() < PER_PAGE;
            all.extend(batch);
            if done {
                break;
            }
            page += 1;
        }
        Ok(all)
    }
}

fn is_rate_limited(resp: &ureq::http::Response<ureq::Body>) -> bool {
    resp.headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "0")
}
