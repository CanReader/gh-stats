use std::collections::HashMap;

use serde::Serialize;

use crate::github::Repo;

#[derive(Debug, PartialEq, Serialize)]
pub struct Language {
    pub name: String,
    pub repos: usize,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub repos: usize,
    pub stars: u32,
    pub forks: u32,
    pub archived: usize,
    pub languages: Vec<Language>,
    pub top: Vec<Repo>,
    pub last_push: Option<Repo>,
}

pub fn collect(repos: &[Repo], include_forks: bool, top_n: usize) -> Stats {
    let repos: Vec<&Repo> = repos.iter().filter(|r| include_forks || !r.fork).collect();

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

    // pushed_at is ISO 8601, so string order is time order
    let last_push = repos
        .iter()
        .filter(|r| r.pushed_at.is_some())
        .max_by(|a, b| a.pushed_at.cmp(&b.pushed_at))
        .map(|r| (*r).clone());

    Stats {
        repos: repos.len(),
        stars: repos.iter().map(|r| r.stargazers_count).sum(),
        forks: repos.iter().map(|r| r.forks_count).sum(),
        archived: repos.iter().filter(|r| r.archived).count(),
        languages,
        top,
        last_push,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

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
            pushed_at: None,
        }
    }

    pub fn sample() -> Vec<Repo> {
        let mut engine = repo("engine", Some("C++"), 4, false);
        engine.pushed_at = Some("2026-09-30T10:00:00Z".into());
        let mut cli = repo("cli", Some("Rust"), 7, false);
        cli.pushed_at = Some("2026-10-02T08:00:00Z".into());
        cli.description = Some("Small command line tool".into());
        let mut notes = repo("notes", None, 0, false);
        notes.archived = true;
        vec![
            engine,
            repo("tool", Some("Rust"), 2, false),
            notes,
            cli,
            repo("someone-elses", Some("Go"), 50, true),
        ]
    }

    #[test]
    fn skips_forks_by_default() {
        let s = collect(&sample(), false, 10);
        assert_eq!(s.repos, 4);
        assert_eq!(s.stars, 13);
        assert_eq!(s.forks, 4);
        assert_eq!(s.archived, 1);
        assert!(s.languages.iter().all(|l| l.name != "Go"));
    }

    #[test]
    fn includes_forks_when_asked() {
        let s = collect(&sample(), true, 10);
        assert_eq!(s.repos, 5);
        assert_eq!(s.stars, 63);
        assert_eq!(s.top[0].name, "someone-elses");
    }

    #[test]
    fn language_shares() {
        let s = collect(&sample(), false, 10);
        let langs: Vec<(&str, usize, f64)> = s
            .languages
            .iter()
            .map(|l| (l.name.as_str(), l.repos, l.percent))
            .collect();
        assert_eq!(langs, [("Rust", 2, 66.7), ("C++", 1, 33.3)]);
    }

    #[test]
    fn top_repos_skip_zero_stars() {
        let s = collect(&sample(), false, 10);
        let names: Vec<&str> = s.top.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["cli", "engine", "tool"]);

        let s = collect(&sample(), false, 1);
        assert_eq!(s.top.len(), 1);
    }

    #[test]
    fn last_push_is_newest() {
        let s = collect(&sample(), false, 5);
        assert_eq!(s.last_push.unwrap().name, "cli");
    }

    #[test]
    fn empty_input() {
        let s = collect(&[], false, 5);
        assert_eq!(s.repos, 0);
        assert_eq!(s.stars, 0);
        assert!(s.languages.is_empty());
        assert!(s.top.is_empty());
        assert!(s.last_push.is_none());
    }
}
