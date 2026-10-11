# gh-stats-cli

Small CLI that shows a GitHub user's repos, stars and languages in the terminal. Uses the GitHub REST API.

```
$ gh-stats CanReader
CanReader (Canberk Pitirli)
repos 58  stars 15  forks 4  followers 19  following 62

languages
  C++          16  ####################
  Rust         14  ##################
  JavaScript    4  #####
  Python        4  #####
  ...

top repos
  FastNN        4 stars  Rust
  Tactix        3 stars  C++
  RustVK        2 stars  Rust
  ...
```

## Install

```
cargo install --git https://github.com/CanReader/gh-stats-cli
```

Or clone it and run `cargo build --release`, the binary is `target/release/gh-stats`.

## Usage

```
gh-stats <user>              summary for a user
gh-stats <user> -t 10        show 10 top repos (default 5)
gh-stats <user> --forks      count forked repos too
gh-stats <user> --json       print JSON, handy for scripts
```

Forks are skipped by default so the numbers are about your own work. Languages are counted by each repo's main language.

## Token

Works without a token but GitHub only allows 60 requests an hour that way. Set `GITHUB_TOKEN` (or `GH_TOKEN`) to get the normal limit:

```
export GITHUB_TOKEN=$(gh auth token)
```

The token only needs read access to public data.

## License

MIT
