//! The `.env.example` template, rendered from the clap argument tree.
//!
//! Every environment variable the server accepts is an `env`-backed argument
//! reachable from [`Cli`](crate::config::Cli). The committed `.env.example` is
//! rendered straight from that tree: each variable's help text becomes its
//! comment, its clap default becomes its value, and a variable with no default is
//! written commented-out and blank (so it is documented without inventing a value
//! — this matters for the ones whose "unset" is a deliberate production behavior,
//! such as the S3 endpoint and credentials).
//!
//! A handful of variables are read outside clap ([`NON_CLAP_KEYS`]); they are
//! appended verbatim. The tests keep the committed file in lockstep with the tree:
//! a new `env` var added anywhere in the config structs shows up automatically,
//! and the drift test fails until the file is regenerated:
//!
//! ```sh
//! ENV_EXAMPLE_WRITE=1 cargo test -p nvisy-cli --all-features env_example
//! ```

use std::path::PathBuf;

use clap::CommandFactory;

use crate::config::Cli;

/// Variables read outside clap (via the tracing `EnvFilter`, not an argument),
/// appended under a trailing "Logging" section as bare `KEY=value` lines.
const NON_CLAP_KEYS: &[(&str, &str)] = &[
    (
        "RUST_LOG",
        "info,nvisy_cli=debug,nvisy_server=debug,nvisy_postgres=debug,nvisy_nats=debug,nvisy_webhook=debug",
    ),
    ("RUST_BACKTRACE", "0"),
];

/// Renders the full `.env.example` text (with a trailing newline) from the clap
/// tree plus the non-clap logging variables.
fn render() -> String {
    let mut out = String::from("# Environment Configuration\n");
    out.push_str("# Copy this file to .env and adjust values as needed.\n");

    let command = Cli::command();
    let mut heading = None;
    render_command(&command, &mut out, &mut heading);

    push_heading(&mut out, "Logging");
    out.push('\n');
    for (key, value) in NON_CLAP_KEYS {
        push_entry(&mut out, key, value);
    }

    out
}

/// Appends every `env`-backed argument of `command` (recursing into subcommands)
/// to `out`, in the tree's flatten order.
///
/// `heading` carries the last-emitted section title across args and recursion, so
/// a `# ─── Section ───` banner is written once whenever the arguments' help
/// heading changes (each config group sets its own via `next_help_heading`).
fn render_command(command: &clap::Command, out: &mut String, heading: &mut Option<String>) {
    for arg in command.get_arguments() {
        let Some(env) = arg.get_env() else { continue };
        let key = env.to_string_lossy();

        let arg_heading = arg.get_help_heading().map(str::to_owned);
        if arg_heading != *heading {
            if let Some(title) = &arg_heading {
                push_heading(out, title);
            }
            *heading = arg_heading;
        }

        out.push('\n');
        let help = arg.get_help().map(ToString::to_string).unwrap_or_default();
        for line in wrap(&sanitize(&help), COMMENT_WIDTH) {
            out.push_str("# ");
            out.push_str(&line);
            out.push('\n');
        }

        match default_value(arg) {
            Some(value) => push_entry(out, &key, &value),
            // No default: document the variable, but leave it commented and blank
            // so its "unset" behavior is preserved.
            None => {
                out.push_str("# ");
                out.push_str(&key);
                out.push_str("=\n");
            }
        }
    }
    for sub in command.get_subcommands() {
        render_command(sub, out, heading);
    }
}

/// Appends a section banner (`# ─── Title ───`), preceded by a blank line.
fn push_heading(out: &mut String, title: &str) {
    out.push_str("\n# ─── ");
    out.push_str(title);
    out.push_str(" ───\n");
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

/// Appends a `KEY=value` line.
fn push_entry(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    out.push_str(value);
    out.push('\n');
}

/// The committed `.env.example`, resolved from this crate's manifest dir so the
/// test works from anywhere in the tree.
fn env_example_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".env.example")
}

/// The committed `.env.example` must match what [`render`] produces from the clap
/// tree. Set `ENV_EXAMPLE_WRITE=1` to rewrite it instead of asserting, so a config
/// change lands as a reviewable diff.
///
/// The committed file is the full one — generated with every feature enabled, so
/// it documents all env vars (the `tls` feature adds `TLS_CERT_PATH`/`TLS_KEY_PATH`).
/// A feature-reduced build renders a subset, so the assertion (and the CI check)
/// run only under `all-features`; regeneration must therefore also use
/// `--all-features`. A reduced build still exercises `render` and the write path.
#[test]
fn env_example_matches_the_clap_tree() {
    let path = env_example_path();
    let rendered = render();

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
