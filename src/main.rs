mod colors;
mod github;
mod render;
mod stats;

use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde::Serialize;

use colors::Palette;
use github::{Client, User};
use stats::Stats;

/// GitHub stats for any user or org, right in your terminal.
#[derive(Parser)]
#[command(name = "gh-stats", version)]
struct Args {
    /// GitHub usernames or orgs (defaults to you, if you're logged in with gh)
    users: Vec<String>,

    /// How many repos to list under "Top repos" (0 hides them)
    #[arg(short, long, default_value_t = 5)]
    top: usize,

    /// Count forked repos too
    #[arg(long)]
    forks: bool,

    /// Print JSON
    #[arg(long, conflicts_with = "md")]
    json: bool,

    /// Print Markdown, ready to paste into a profile README
    #[arg(long)]
    md: bool,

    /// When to use colors
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,
}

#[derive(Clone, Copy, ValueEnum)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Serialize)]
struct Report {
    user: User,
    #[serde(flatten)]
    stats: Stats,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let client = Client::new(github::find_token());

    let users = match args.users.is_empty() {
        true => match client.me() {
            Ok(me) => vec![me],
            Err(e) => {
                eprintln!("gh-stats: {e:#}");
                return ExitCode::FAILURE;
            }
        },
        false => args.users.clone(),
    };

    let mut reports = Vec::new();
    let mut failed = false;
    for login in &users {
        match load(&client, login, &args) {
            Ok(r) => reports.push(r),
            Err(e) => {
                eprintln!("gh-stats: {e:#}");
                failed = true;
            }
        }
    }

    if args.json {
        let json = match reports.as_slice() {
            [one] => serde_json::to_string_pretty(one),
            many => serde_json::to_string_pretty(many),
        };
        match json {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("gh-stats: {e}");
                failed = true;
            }
        }
    } else {
        let palette = palette(args.color);
        let width = terminal_width();
        let blocks: Vec<String> = reports
            .iter()
            .map(|r| match args.md {
                true => render::markdown(&r.user, &r.stats),
                false => render::text(&r.user, &r.stats, palette, width),
            })
            .collect();
        print!("{}", blocks.join("\n"));
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn load(client: &Client, login: &str, args: &Args) -> Result<Report> {
    let user = client.user(login)?;
    let repos = client.repos(&user.login)?;
    let stats = stats::collect(&repos, args.forks, args.top);
    Ok(Report { user, stats })
}

fn palette(mode: ColorMode) -> Palette {
    let on = match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
    };
    let truecolor = std::env::var("COLORTERM")
        .map(|v| v.contains("truecolor") || v.contains("24bit"))
        .unwrap_or(false);
    match (on, truecolor) {
        (false, _) => Palette::None,
        (true, true) => Palette::TrueColor,
        (true, false) => Palette::Ansi256,
    }
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(100)
        .clamp(40, 160)
}
