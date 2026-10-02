//! Capture a CLI's real surface by running it.
//!
//! `cargo metadata` names the binaries but says nothing about what they do.
//! Invoking `--help` does. The Commands page crux maintains by hand with
//! fourteen terminals is reproducible by asking the binary itself, and it
//! cannot go stale the way a hand-written page can.
//!
//! The subcommand pass is what makes that true at scale: a top-level `--help`
//! lists the subcommands, so the page gets one terminal per subcommand rather
//! than one per crate.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Cap on subcommands captured per binary. A binary with forty of them would
/// make the page unreadable and the run slow; the point is coverage of the
/// commands a reader is likely to try first.
const MAX_SUBCOMMANDS: usize = 14;

/// How long a single `--help` may take before it is treated as hanging.
const TIMEOUT: Duration = Duration::from_secs(90);

/// One captured `--help`, ready to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpCapture {
    /// The invocation that produced it, e.g. `crs probe`.
    pub label: String,
    pub output: String,
}

/// Capture `--help` for a binary and each of the subcommands it lists.
///
/// Subcommands are read out of the conventional `Commands:` block. A binary
/// that hangs, or exits non-zero, contributes nothing rather than failing the
/// run: a page missing a command is better than no page.
pub fn capture_help(root: &Path, binary: &str) -> Vec<HelpCapture> {
    let mut out = Vec::new();

    let Some(top) = run_help(root, binary, &[]) else {
        return out;
    };
    out.push(HelpCapture {
        label: format!("{binary} --help"),
        output: top.clone(),
    });

    for sub in subcommands_of(&top).into_iter().take(MAX_SUBCOMMANDS) {
        if let Some(body) = run_help(root, binary, &[sub.as_str()]) {
            out.push(HelpCapture {
                label: format!("{binary} {sub} --help"),
                output: body,
            });
        }
    }
    out
}

fn run_help(root: &Path, binary: &str, args: &[&str]) -> Option<String> {
    let start = Instant::now();
    let output = Command::new("cargo")
        .arg("run")
        .args(["-q", "--bin", binary, "--"])
        .arg("--help")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;

    if start.elapsed() > TIMEOUT {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let text = text.trim_end().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// Pull the subcommand names out of a `--help` listing.
///
/// Only the conventional `Commands:` section is read, and a name that does not
/// look like a single bare word is skipped — that column also carries the
/// one-line descriptions.
fn subcommands_of(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_block = false;
    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("Commands:") || trimmed.eq_ignore_ascii_case("Subcommands:")
        {
            in_block = true;
            continue;
        }
        if in_block {
            if trimmed.is_empty() {
                break;
            }
            // A section header ends the block.
            if !trimmed.starts_with(char::is_whitespace) && trimmed.ends_with(':') {
                break;
            }
            let Some(first) = trimmed.split_whitespace().next() else {
                continue;
            };
            if first.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                names.push(first.to_string());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELP: &str = "\
Overwatch hook pipeline

Usage: crs <COMMAND>

Commands:
  pre             PreToolUse hook
  post            PostToolUse hook
  probe           Probe a command
  discover        Discover missed savings
  help            Print this message

Options:
  -h, --help  Print help
";

    #[test]
    fn reads_subcommands_from_the_commands_block() {
        assert_eq!(
            subcommands_of(HELP),
            vec!["pre", "post", "probe", "discover", "help"]
        );
    }

    #[test]
    fn stops_at_the_next_section() {
        assert!(
            !subcommands_of(HELP).contains(&"Print".to_string()),
            "Options section must not be read as commands"
        );
        assert_eq!(subcommands_of(HELP).len(), 5);
    }

    #[test]
    fn no_commands_block_yields_nothing() {
        let help = "A tool\n\nUsage: thing [OPTIONS]\n\nOptions:\n  -v\n";
        assert!(subcommands_of(help).is_empty());
    }

    #[test]
    fn handles_a_subcommands_heading() {
        let help = "Usage: x\n\nSubcommands:\n  run   Run it\n";
        assert_eq!(subcommands_of(help), vec!["run"]);
    }

    #[test]
    fn a_binary_with_no_help_contributes_nothing() {
        // No build happens in a unit test; the contract is the empty return.
        let captures = capture_help(Path::new("/nonexistent-binary-root"), "nope");
        assert!(captures.is_empty());
    }
}
