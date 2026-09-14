//! The `.env.example` template, rendered from the clap argument tree.
//!
//! Every environment variable the server accepts is an `env`-backed argument
//! reachable from [`Cli`]. The committed `.env.example` is rendered straight from
//! that tree: each variable's help text becomes its comment, its clap default
//! becomes its value, and a variable with no default is written commented-out and
//! blank (so it is documented without inventing a value — this matters for the
//! ones whose "unset" is a deliberate production behavior, such as the S3 endpoint
//! and credentials).
//!
//! A handful of variables are read outside clap ([`NON_CLAP_VARS`]); they are
//! declared with the same section/comment/value shape and rendered through the
//! same path. The tests keep the committed file in lockstep with the tree:
//! a new `env` var added anywhere in the config structs shows up automatically,
//! and the drift test fails until the file is regenerated:
//!
//! ```sh
//! ENV_EXAMPLE_WRITE=1 cargo test -p nvisy-cli --all-features env_example
//! ```
//!
//! [`Cli`]: crate::config::Cli

use std::path::PathBuf;

use clap::CommandFactory;

use crate::config::Cli;

/// A variable the server reads outside clap (so it is not an `env`-backed
/// argument and cannot be discovered from the [`Cli`] tree), described here so it
/// still renders into `.env.example` with a section, comment, and value like every
/// other variable.
struct NonClapVar {
    /// Section banner this variable is grouped under.
    section: &'static str,
    /// One-line description, rendered as its `#` comment.
    comment: &'static str,
    /// The environment variable name.
    key: &'static str,
    /// The default value written into the file.
    value: &'static str,
}

/// Variables read outside clap — currently the tracing/runtime knobs consumed by
/// [`tracing_subscriber::EnvFilter`] and the Rust runtime, not by an argument.
/// Rendered through the same path as the clap-derived variables.
const NON_CLAP_VARS: &[NonClapVar] = &[
    NonClapVar {
        section: "Logging",
        comment: "Tracing filter: comma-separated `target=level` directives.",
        key: "RUST_LOG",
        value: "info,nvisy_cli=debug,nvisy_server=debug,nvisy_postgres=debug,nvisy_nats=debug,nvisy_webhook=debug",
    },
    NonClapVar {
        section: "Logging",
        comment: "Panic backtraces: `0` off, `1` on, `full` for full (verbose) traces.",
        key: "RUST_BACKTRACE",
        value: "0",
    },
];

/// Accumulates the `.env.example` text, tracking the current section so a banner
/// is written only when it changes. Clap-derived and non-clap variables both go
/// through [`var`], so they format identically.
///
/// [`var`]: Renderer::var
struct Renderer {
    out: String,
    section: Option<String>,
}

impl Renderer {
    /// Creates an empty renderer.
    fn new() -> Self {
        Self {
            out: String::new(),
            section: None,
        }
    }

    /// Renders the whole file: the header (which points at `make generate-env`),
    /// every clap `env` argument, then the non-clap vars.
    fn render(mut self) -> String {
        self.out.push_str("# Environment Configuration\n");
        self.out
            .push_str("# Run `make generate-env` to create or update your .env from this file\n");
        self.out
            .push_str("# (adds new keys, keeps your existing values), then adjust as needed.\n");

        self.command(&Cli::command());

        for var in NON_CLAP_VARS {
            self.var(
                Some(var.section),
                var.comment,
                &format!("{}={}", var.key, var.value),
            );
        }

        self.out
    }

    /// Renders every `env`-backed argument of `command` (recursing into
    /// subcommands), in the tree's flatten order.
    fn command(&mut self, command: &clap::Command) {
        for arg in command.get_arguments() {
            let Some(env) = arg.get_env() else { continue };
            let key = env.to_string_lossy();
            let help = arg.get_help().map(ToString::to_string).unwrap_or_default();

            // A var with a clap default renders `KEY=value`; one without is written
            // commented-out and blank, so it is documented without inventing a value
            // (its "unset" behavior is preserved).
            let entry = match default_value(arg) {
                Some(value) => format!("{key}={value}"),
                None => format!("# {key}="),
            };

            self.var(arg.get_help_heading(), &help, &entry);
        }
        for sub in command.get_subcommands() {
            self.command(sub);
        }
    }

    /// Renders one variable: its section banner (only when `section` differs from
    /// the last one written), its wrapped `#` comment, and its pre-formatted
    /// `entry` line.
    fn var(&mut self, section: Option<&str>, comment: &str, entry: &str) {
        if section != self.section.as_deref() {
            if let Some(title) = section {
                self.out.push_str("\n# ─── ");
                self.out.push_str(title);
                self.out.push_str(" ───\n");
            }
            self.section = section.map(str::to_owned);
        }

        self.out.push('\n');
        for line in wrap(&sanitize(comment), COMMENT_WIDTH) {
            self.out.push_str("# ");
            self.out.push_str(&line);
            self.out.push('\n');
        }
        self.out.push_str(entry);
        self.out.push('\n');
    }
}

/// The argument's default rendered as an env value, joining a multi-value default
/// (e.g. a comma-delimited list) with the arg's own delimiter. `None` when the arg
/// has no default.
fn default_value(arg: &clap::Arg) -> Option<String> {
    let defaults = arg.get_default_values();
    if defaults.is_empty() {
        return None;
    }
    let delimiter = arg.get_value_delimiter().unwrap_or(',');
    let joined = defaults
        .iter()
        .map(|v| v.to_string_lossy())
        .collect::<Vec<_>>()
        .join(&delimiter.to_string());
    Some(joined)
}

/// Column to wrap comment prose at, so no comment line becomes an unreadable wall.
const COMMENT_WIDTH: usize = 88;

/// Word-wraps `text` to at most `width` columns per line, breaking only on spaces
/// (a word longer than `width`, such as a URL, is left whole on its own line). A
/// blank line in the input is preserved as a paragraph break.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
            } else if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// Cleans help text for a config file: strips rustdoc intra-doc link markup
/// (`[`X`](path)` → `X`), drops inline-code backticks, and preserves paragraph
/// breaks (a blank line in the doc-comment) while otherwise leaving the prose as
/// authored. A leaked link or stray backtick is a defect in the source
/// doc-comment; this keeps it out of the committed file regardless.
fn sanitize(help: &str) -> String {
    let mut out = String::with_capacity(help.len());
    let mut chars = help.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // `[`Name`](target)` or `[text](target)` → keep the visible text only.
            '[' => {
                let mut text = String::new();
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                    if c != '`' {
                        text.push(c);
                    }
                }
                // Drop an immediately following `(...)` link target.
                if chars.peek() == Some(&'(') {
                    for c in chars.by_ref() {
                        if c == ')' {
                            break;
                        }
                    }
                }
                out.push_str(&text);
            }
            // Inline-code backticks read as noise in a plain env comment.
            '`' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// The committed `.env.example`, resolved from this crate's manifest dir so the
/// test works from anywhere in the tree.
fn env_example_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".env.example")
}

/// The committed `.env.example` must match what [`Renderer`] produces from the
/// clap tree. Set `ENV_EXAMPLE_WRITE=1` to rewrite it instead of asserting, so a
/// config change lands as a reviewable diff.
///
/// The committed file is the full one — generated with every feature enabled, so
/// it documents all env vars (the `tls` feature adds `TLS_CERT_PATH`/`TLS_KEY_PATH`).
/// A feature-reduced build renders a subset, so the assertion (and the CI check)
/// run only under `all-features`; regeneration must therefore also use
/// `--all-features`. A reduced build still exercises the renderer and the write path.
#[test]
fn env_example_matches_the_clap_tree() {
    let path = env_example_path();
    let rendered = Renderer::new().render();

    if std::env::var_os("ENV_EXAMPLE_WRITE").is_some() {
        std::fs::write(&path, &rendered).expect("write .env.example");
        return;
    }

    // Only the full-feature render is the committed superset; skip the comparison
    // otherwise so a partial-feature build does not fail on the vars it omits.
    if !cfg!(feature = "tls") {
        return;
    }

    let committed = std::fs::read_to_string(&path).expect("read .env.example");
    assert_eq!(
        committed, rendered,
        "\n.env.example is out of date with the config structs; \
         regenerate with: ENV_EXAMPLE_WRITE=1 cargo test -p nvisy-cli --all-features env_example\n"
    );
}
