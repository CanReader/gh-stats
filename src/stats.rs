use std::collections::HashMap;

use serde::Serialize;

use crate::github::Repo;

const ACTIVE_DAYS: i64 = 30;

#[derive(Debug, PartialEq, Serialize)]
pub struct Language {
    pub name: String,
    pub repos: usize,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    /// Everything, same number GitHub shows on the profile
    pub total: usize,
    pub own: usize,
    pub fork_repos: usize,
    /// Repos the rest of the stats are based on
    pub repos: usize,
    pub public: usize,
    pub private: usize,
    pub stars: u32,
    pub forks: u32,
    pub archived: usize,
    pub active_last_30_days: usize,
    pub languages: Vec<Language>,
    pub top: Vec<Repo>,
    pub last_push: Option<Repo>,
    pub newest: Option<Repo>,
}

pub fn collect(all: &[Repo], include_forks: bool, top_n: usize, now_secs: u64) -> Stats {
    let repos: Vec<&Repo> = all.iter().filter(|r| include_forks || !r.fork).collect();
    let today = now_secs as i64 / 86_400;

    let mut langs: HashMap<&str, usize> = HashMap::new();
    for r in &repos {
        if let Some(lang) = &r.language {
            *langs.entry(lang).or_default() += 1;
        }
    }
    let with_lang: usize = langs.values().sum();
    let mut languages: Vec<Language> = langs
        .into_iter()
        .map(|(name, n)| Language {
            name: name.to_string(),
            repos: n,
            percent: (n as f64 * 1000.0 / with_lang as f64).round() / 10.0,
        })
        .collect();
    languages.sort_by(|a, b| b.repos.cmp(&a.repos).then_with(|| a.name.cmp(&b.name)));

    let mut top: Vec<Repo> = repos
        .iter()
        .filter(|r| r.stargazers_count > 0)
        .map(|r| (*r).clone())
        .collect();
    top.sort_by(|a, b| {
        b.stargazers_count
            .cmp(&a.stargazers_count)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    top.truncate(top_n);

    // ISO 8601 dates, so string order is time order
    let latest_by = |key: fn(&Repo) -> &Option<String>| {
        repos
            .iter()
            .filter(|r| key(r).is_some())
            .max_by(|a, b| key(a).cmp(key(b)))
            .map(|r| (*r).clone())
    };

    let active = repos
        .iter()
        .filter_map(|r| r.pushed_at.as_deref().and_then(days))
        .filter(|d| today - d <= ACTIVE_DAYS)
        .count();

    let own: Vec<&Repo> = all.iter().filter(|r| !r.fork).collect();

    Stats {
        total: all.len(),
        own: own.len(),
        fork_repos: all.len() - own.len(),
        repos: repos.len(),
        public: own.iter().filter(|r| !r.private).count(),
        private: own.iter().filter(|r| r.private).count(),
        stars: repos.iter().map(|r| r.stargazers_count).sum(),
        forks: repos.iter().map(|r| r.forks_count).sum(),
        archived: repos.iter().filter(|r| r.archived).count(),
        active_last_30_days: active,
        languages,
        top,
        last_push: latest_by(|r| &r.pushed_at),
        newest: latest_by(|r| &r.created_at),
    }
}

/// Days since 1970-01-01 for a date like "2026-10-11T08:00:00Z".
fn days(iso: &str) -> Option<i64> {
    let y: i64 = iso.get(0..4)?.parse().ok()?;
    let m: i64 = iso.get(5..7)?.parse().ok()?;
    let d: i64 = iso.get(8..10)?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // 2026-10-05T00:00:00Z
    pub const NOW: u64 = 1_791_158_400;

    pub fn repo(name: &str, lang: Option<&str>, stars: u32, fork: bool) -> Repo {
        Repo {
            name: name.to_string(),
            html_url: format!("https://github.com/someone/{name}"),
            description: None,
            language: lang.map(String::from),
            stargazers_count: stars,
            forks_count: 1,
            fork,
            archived: false,
            private: false,
            created_at: None,
            pushed_at: None,
        }
    }

    pub fn sample() -> Vec<Repo> {
        let mut engine = repo("engine", Some("C++"), 4, false);
        engine.pushed_at = Some("2026-08-30T10:00:00Z".into());
        engine.created_at = Some("2024-01-10T10:00:00Z".into());
        let mut cli = repo("cli", Some("Rust"), 7, false);
        cli.pushed_at = Some("2026-10-02T08:00:00Z".into());
        cli.created_at = Some("2025-06-01T08:00:00Z".into());
        cli.description = Some("Small command line tool".into());
        let mut notes = repo("notes", None, 0, false);
        notes.archived = true;
        let mut secret = repo("secret", Some("Rust"), 0, false);
        secret.private = true;
        secret.pushed_at = Some("2026-09-20T08:00:00Z".into());
        secret.created_at = Some("2026-09-01T08:00:00Z".into());
        vec![
            engine,
            repo("tool", Some("Rust"), 2, false),
            notes,
            cli,
            secret,
            repo("someone-elses", Some("Go"), 50, true),
        ]
    }

    #[test]
    fn days_since_epoch() {
        assert_eq!(days("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(days("2000-03-01"), Some(11_017));
        assert_eq!(days("2026-10-05T00:00:00Z"), Some(NOW as i64 / 86_400));
        assert_eq!(days("garbage"), None);
    }

    #[test]
    fn skips_forks_by_default() {
        let s = collect(&sample(), false, 10, NOW);
        assert_eq!((s.total, s.own, s.fork_repos), (6, 5, 1));
        assert_eq!(s.repos, 5);
        assert_eq!(s.stars, 13);
        assert_eq!(s.forks, 5);
        assert_eq!(s.archived, 1);
        assert!(s.languages.iter().all(|l| l.name != "Go"));
    }

    #[test]
    fn includes_forks_when_asked() {
        let s = collect(&sample(), true, 10, NOW);
        assert_eq!(s.repos, 6);
        assert_eq!((s.total, s.own, s.fork_repos), (6, 5, 1));
        assert_eq!(s.stars, 63);
        assert_eq!(s.top[0].name, "someone-elses");
    }

    #[test]
    fn public_and_private() {
        let s = collect(&sample(), false, 10, NOW);
        assert_eq!((s.public, s.private), (4, 1));
    }

    #[test]
    fn language_shares() {
        let s = collect(&sample(), false, 10, NOW);
        let langs: Vec<(&str, usize, f64)> = s
            .languages
            .iter()
            .map(|l| (l.name.as_str(), l.repos, l.percent))
            .collect();
        assert_eq!(langs, [("Rust", 3, 75.0), ("C++", 1, 25.0)]);
    }

    #[test]
    fn top_repos_skip_zero_stars() {
        let s = collect(&sample(), false, 10, NOW);
        let names: Vec<&str> = s.top.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["cli", "engine", "tool"]);

        let s = collect(&sample(), false, 1, NOW);
        assert_eq!(s.top.len(), 1);
    }

    #[test]
    fn activity() {
        let s = collect(&sample(), false, 5, NOW);
        assert_eq!(s.last_push.unwrap().name, "cli");
        assert_eq!(s.newest.unwrap().name, "secret");
        // cli and secret are within 30 days, engine is not
        assert_eq!(s.active_last_30_days, 2);
    }

    #[test]
    fn empty_input() {
        let s = collect(&[], false, 5, NOW);
        assert_eq!(s.repos, 0);
        assert_eq!(s.stars, 0);
        assert!(s.languages.is_empty());
        assert!(s.top.is_empty());
        assert!(s.last_push.is_none());
        assert!(s.newest.is_none());
    }
}
