use std::collections::HashMap;

use serde::Serialize;

use crate::github::Repo;

#[derive(Debug, PartialEq, Serialize)]
pub struct Language {
    pub name: String,
    pub repos: usize,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub repos: usize,
    pub stars: u32,
    pub forks: u32,
    pub languages: Vec<Language>,
    pub top: Vec<Repo>,
}

pub fn collect(repos: &[Repo], include_forks: bool, top_n: usize) -> Stats {
    let repos: Vec<&Repo> = repos.iter().filter(|r| include_forks || !r.fork).collect();

    let mut langs: HashMap<&str, usize> = HashMap::new();
    for r in &repos {
        if let Some(lang) = &r.language {
            *langs.entry(lang).or_default() += 1;
        }
    }
    let mut languages: Vec<Language> = langs
        .into_iter()
        .map(|(name, repos)| Language {
            name: name.to_string(),
            repos,
        })
        .collect();
    languages.sort_by(|a, b| b.repos.cmp(&a.repos).then_with(|| a.name.cmp(&b.name)));

    let mut top: Vec<Repo> = repos.iter().map(|r| (*r).clone()).collect();
    top.sort_by(|a, b| {
        b.stargazers_count
            .cmp(&a.stargazers_count)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    top.truncate(top_n);

    Stats {
        repos: repos.len(),
        stars: repos.iter().map(|r| r.stargazers_count).sum(),
        forks: repos.iter().map(|r| r.forks_count).sum(),
        languages,
        top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str, lang: Option<&str>, stars: u32, fork: bool) -> Repo {
        Repo {
            name: name.to_string(),
            description: None,
            language: lang.map(String::from),
            stargazers_count: stars,
            forks_count: 1,
            fork,
            archived: false,
        }
    }

    fn sample() -> Vec<Repo> {
        vec![
            repo("engine", Some("C++"), 4, false),
            repo("tool", Some("Rust"), 2, false),
            repo("notes", None, 0, false),
            repo("cli", Some("Rust"), 7, false),
            repo("someone-elses", Some("Go"), 50, true),
        ]
    }

    #[test]
    fn skips_forks_by_default() {
        let s = collect(&sample(), false, 10);
        assert_eq!(s.repos, 4);
        assert_eq!(s.stars, 13);
        assert_eq!(s.forks, 4);
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
    fn languages_sorted_by_count() {
        let s = collect(&sample(), false, 10);
        let langs: Vec<(&str, usize)> = s
            .languages
            .iter()
            .map(|l| (l.name.as_str(), l.repos))
            .collect();
        assert_eq!(langs, [("Rust", 2), ("C++", 1)]);
    }

    #[test]
    fn top_repos_by_stars() {
        let s = collect(&sample(), false, 2);
        let names: Vec<&str> = s.top.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["cli", "engine"]);
    }

    #[test]
    fn empty_input() {
        let s = collect(&[], false, 5);
        assert_eq!(s.repos, 0);
        assert_eq!(s.stars, 0);
        assert!(s.languages.is_empty());
        assert!(s.top.is_empty());
    }
}
