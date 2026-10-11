use std::fmt::Write;

use crate::colors::{self, Palette};
use crate::github::User;
use crate::stats::Stats;

const ACCENT: (u8, u8, u8) = (0x58, 0xA6, 0xFF);
const STAR: (u8, u8, u8) = (0xE3, 0xB3, 0x41);
const MAX_LANGUAGES: usize = 8;
const BAR_CELLS: usize = 24;

pub fn text(user: &User, stats: &Stats, p: Palette, width: usize) -> String {
    let mut out = String::new();

    let mut head = format!("  {}", p.bold(&p.fg(ACCENT, &user.login)));
    if let Some(name) = user.name.as_deref().filter(|n| !n.is_empty()) {
        head += &format!("  {name}");
    }
    let kind = if user.kind == "Organization" {
        "org"
    } else {
        "user"
    };
    let since = user.created_at.get(..4).unwrap_or("?");
    let _ = writeln!(out, "{head}");
    let _ = writeln!(
        out,
        "  {}\n",
        p.dim(&format!(
            "{kind} since {since}, {} followers, {} following",
            num(user.followers),
            num(user.following)
        ))
    );

    let mut totals = format!(
        "  {} repos   {} stars   forked {}",
        p.bold(&num(stats.total as u32)),
        p.bold(&p.fg(STAR, &num(stats.stars))),
        p.bold(&plural(stats.forks as usize, "time")),
    );
    if stats.archived > 0 {
        totals += &format!("   {} archived", stats.archived);
    }
    let _ = writeln!(out, "{totals}");

    let mut split = match stats.private {
        0 => format!("{} own", stats.own),
        n => format!("{} own ({} public, {n} private)", stats.own, stats.public),
    };
    if stats.fork_repos > 0 {
        split += &format!(", {}", plural(stats.fork_repos, "fork"));
        if stats.repos < stats.total {
            split += " left out of the stats (--forks adds them)";
        }
    }
    if stats.private > 0 || stats.fork_repos > 0 {
        let _ = writeln!(out, "  {}", p.dim(&split));
    }

    if let Some(r) = &stats.last_push {
        let _ = writeln!(
            out,
            "  {}",
            p.dim(&format!(
                "last push: {} on {}, {} active in the last 30 days",
                r.name,
                date(&r.pushed_at),
                plural(stats.active_last_30_days, "repo")
            ))
        );
    }
    if let Some(r) = &stats.newest {
        let _ = writeln!(
            out,
            "  {}",
            p.dim(&format!(
                "newest: {}, created {}",
                r.name,
                date(&r.created_at)
            ))
        );
    }

    if !stats.languages.is_empty() {
        let _ = writeln!(out, "\n  {}", p.bold("Languages"));
        let shown = &stats.languages[..stats.languages.len().min(MAX_LANGUAGES)];
        let name_w = shown
            .iter()
            .map(|l| l.name.chars().count())
            .max()
            .unwrap_or(0);
        let max = shown[0].repos;
        for l in shown {
            let color = colors::language(&l.name);
            let bar = format!("{:<BAR_CELLS$}", bar(l.repos, max, BAR_CELLS));
            let _ = writeln!(
                out,
                "  {} {:<name_w$}  {}  {:>3}  {:>5.1}%",
                p.fg(color, "●"),
                l.name,
                p.fg(color, &bar),
                l.repos,
                l.percent
            );
        }
    }

    if !stats.top.is_empty() {
        let _ = writeln!(out, "\n  {}", p.bold("Top repos"));
        let star_w = num(stats.top[0].stargazers_count).len();
        let name_w = stats
            .top
            .iter()
            .map(|r| r.name.chars().count())
            .max()
            .unwrap_or(0);
        let lang_w = stats
            .top
            .iter()
            .map(|r| r.language.as_deref().unwrap_or("-").chars().count())
            .max()
            .unwrap_or(0);
        let used = 2 + 2 + star_w + 2 + name_w + 2 + lang_w + 2;
        for r in &stats.top {
            let lang = r.language.as_deref().unwrap_or("-");
            let mut line = format!(
                "  {} {:>star_w$}  {:<name_w$}  {}",
                p.fg(STAR, "★"),
                num(r.stargazers_count),
                r.name,
                p.fg(colors::language(lang), &format!("{lang:<lang_w$}")),
            );
            let mut desc = strip_shortcodes(r.description.as_deref().unwrap_or(""));
            if r.private {
                desc = format!("(private) {desc}").trim_end().to_string();
            }
            let room = width.saturating_sub(used);
            if !desc.is_empty() && room >= 12 {
                line += &format!("  {}", p.dim(&truncate(&desc, room)));
            }
            let _ = writeln!(out, "{}", line.trim_end());
        }
    }

    out
}

pub fn markdown(user: &User, stats: &Stats) -> String {
    let mut out = String::new();
    let name = user
        .name
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or(&user.login);
    let _ = writeln!(out, "### {name} ([@{}]({}))\n", user.login, user.html_url);
    let _ = writeln!(
        out,
        "**{}** repos · **{}** stars · forked **{}** times · **{}** followers\n",
        stats.total, stats.stars, stats.forks, user.followers
    );

    if !stats.languages.is_empty() {
        out += "| Language | Repos | Share |\n| --- | ---: | ---: |\n";
        for l in stats.languages.iter().take(MAX_LANGUAGES) {
            let _ = writeln!(out, "| {} | {} | {:.1}% |", l.name, l.repos, l.percent);
        }
        out.push('\n');
    }

    if !stats.top.is_empty() {
        out += "| Repo | Stars | Language | About |\n| --- | ---: | --- | --- |\n";
        for r in &stats.top {
            let desc = r.description.as_deref().unwrap_or("");
            let _ = writeln!(
                out,
                "| [{}]({}) | {} | {} | {} |",
                r.name,
                r.html_url,
                r.stargazers_count,
                r.language.as_deref().unwrap_or("-"),
                md_cell(&truncate(desc, 80)),
            );
        }
        out.push('\n');
    }

    out += "<sub>made with [gh-stats](https://github.com/CanReader/gh-stats)</sub>\n";
    out
}

fn bar(value: usize, max: usize, cells: usize) -> String {
    const PARTS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    if max == 0 || value == 0 {
        return String::new();
    }
    let eighths = ((value * cells * 8 + max / 2) / max).max(1);
    let mut s = "█".repeat(eighths / 8);
    let rest = eighths % 8;
    if rest > 0 {
        s.push(PARTS[rest]);
    }
    s
}

// GitHub shows :crab: as an emoji, a terminal can't
fn strip_shortcodes(s: &str) -> String {
    s.split_whitespace()
        .filter(|w| {
            let inner = w.strip_prefix(':').and_then(|w| w.strip_suffix(':'));
            !inner.is_some_and(|i| {
                !i.is_empty()
                    && i.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_+-".contains(c))
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn date(iso: &Option<String>) -> &str {
    iso.as_deref().and_then(|d| d.get(..10)).unwrap_or("?")
}

fn plural(n: usize, word: &str) -> String {
    match n {
        1 => format!("1 {word}"),
        n => format!("{} {word}s", num(n as u32)),
    }
}

fn num(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t = t.trim_end().to_string();
    t.push('…');
    t
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{
        collect,
        tests::{sample, NOW},
    };

    fn user() -> User {
        User {
            login: "someone".into(),
            name: Some("Some One".into()),
            kind: "User".into(),
            html_url: "https://github.com/someone".into(),
            public_repos: 5,
            followers: 12,
            following: 3,
            created_at: "2019-04-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn bars() {
        assert_eq!(bar(4, 4, 4), "████");
        assert_eq!(bar(2, 4, 4), "██");
        assert_eq!(bar(1, 3, 4), "█▍");
        assert_eq!(bar(1, 1000, 4), "▏");
        assert_eq!(bar(0, 4, 4), "");
        assert_eq!(bar(3, 0, 4), "");
    }

    #[test]
    fn shortcodes() {
        assert_eq!(
            strip_shortcodes(":crab: Small exercises"),
            "Small exercises"
        );
        assert_eq!(strip_shortcodes("fast :zap: and  safe"), "fast and safe");
        assert_eq!(strip_shortcodes("ratio 1:2 is fine"), "ratio 1:2 is fine");
        assert_eq!(strip_shortcodes("::"), "::");
    }

    #[test]
    fn plurals() {
        assert_eq!(plural(1, "fork"), "1 fork");
        assert_eq!(plural(0, "fork"), "0 forks");
        assert_eq!(plural(68553, "time"), "68,553 times");
    }

    #[test]
    fn number_grouping() {
        assert_eq!(num(0), "0");
        assert_eq!(num(999), "999");
        assert_eq!(num(1000), "1,000");
        assert_eq!(num(400838), "400,838");
        assert_eq!(num(1234567), "1,234,567");
    }

    #[test]
    fn truncating() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a long description here", 10), "a long de…");
        assert_eq!(truncate("çok güzel bir şey", 6), "çok g…");
    }

    #[test]
    fn plain_text_report() {
        let stats = collect(&sample(), false, 5, NOW);
        let out = text(&user(), &stats, Palette::None, 80);
        assert!(!out.contains('\x1b'));
        assert!(out.contains("someone  Some One"));
        assert!(out.contains("user since 2019, 12 followers, 3 following"));
        assert!(out.contains("6 repos   13 stars   forked 5 times   1 archived"));
        assert!(out.contains("5 own (4 public, 1 private), 1 fork left out of the stats"));
        assert!(out.contains("last push: cli on 2026-10-02, 2 repos active in the last 30 days"));
        assert!(out.contains("newest: secret, created 2026-09-01"));
        assert!(out.contains("● Rust"));
        assert!(out.contains("75.0%"));
        assert!(out.contains("★ 7  cli"));
        assert!(out.contains("Small command line tool"));
    }

    #[test]
    fn narrow_terminal_drops_descriptions() {
        let stats = collect(&sample(), false, 5, NOW);
        let out = text(&user(), &stats, Palette::None, 20);
        assert!(!out.contains("Small command"));
    }

    #[test]
    fn colored_report_has_escape_codes() {
        let stats = collect(&sample(), false, 5, NOW);
        let out = text(&user(), &stats, Palette::TrueColor, 80);
        assert!(out.contains("\x1b[38;2;"));
    }

    #[test]
    fn markdown_report() {
        let mut repos = sample();
        repos[3].description = Some("pipes | in | text".into());
        repos.retain(|r| !r.private);
        let stats = collect(&repos, false, 5, NOW);
        let md = markdown(&user(), &stats);
        assert!(md.starts_with("### Some One ([@someone](https://github.com/someone))"));
        assert!(md.contains("**5** repos · **13** stars · forked **4** times"));
        assert!(md.contains("| Rust | 2 | 66.7% |"));
        assert!(md.contains(
            "| [cli](https://github.com/someone/cli) | 7 | Rust | pipes \\| in \\| text |"
        ));
    }
}
