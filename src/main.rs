mod github;
mod stats;

use anyhow::Result;
use clap::Parser;
use serde::Serialize;

use github::{Client, User};
use stats::Stats;

/// Show a GitHub user's repos, stars and languages.
#[derive(Parser)]
#[command(name = "gh-stats", version)]
struct Args {
    /// GitHub username
    user: String,

    /// How many repos to list in "top repos"
    #[arg(short, long, default_value_t = 5)]
    top: usize,

    /// Count forked repos too
    #[arg(long)]
    forks: bool,

    /// Print JSON instead of the table
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct Report<'a> {
    user: &'a User,
    #[serde(flatten)]
    stats: &'a Stats,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok()
        .filter(|t| !t.is_empty());

    let client = Client::new(token);
    let user = client.user(&args.user)?;
    let repos = client.repos(&args.user)?;
    let stats = stats::collect(&repos, args.forks, args.top);

    if args.json {
        let report = Report {
            user: &user,
            stats: &stats,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&user, &stats);
    }
    Ok(())
}

fn print_report(user: &User, stats: &Stats) {
    match &user.name {
        Some(name) => println!("{} ({name})", user.login),
        None => println!("{}", user.login),
    }
    println!(
        "repos {}  stars {}  forks {}  followers {}  following {}",
        stats.repos, stats.stars, stats.forks, user.followers, user.following
    );

    if !stats.languages.is_empty() {
        println!("\nlanguages");
        let width = stats
            .languages
            .iter()
            .map(|l| l.name.len())
            .max()
            .unwrap_or(0);
        let max = stats.languages[0].repos;
        for lang in stats.languages.iter().take(8) {
            let bar = "#".repeat((lang.repos * 20).div_ceil(max));
            println!("  {:<width$}  {:>3}  {bar}", lang.name, lang.repos);
        }
    }

    if !stats.top.is_empty() {
        println!("\ntop repos");
        let width = stats.top.iter().map(|r| r.name.len()).max().unwrap_or(0);
        for r in &stats.top {
            let lang = r.language.as_deref().unwrap_or("-");
            let archived = if r.archived { "  (archived)" } else { "" };
            let stars = match r.stargazers_count {
                1 => "1 star ".to_string(),
                n => format!("{n} stars"),
            };
            println!("  {:<width$}  {stars:>9}  {lang}{archived}", r.name);
        }
    }
}
