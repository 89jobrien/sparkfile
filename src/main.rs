use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

use sparkfile::{
    domain::{Preset, ProjectSpec, SpecError},
    fs::{WriteError, write_files},
    scaffold::{ScaffoldError, generate},
};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(summary) => {
            print_summary(&summary);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!();
            eprintln!("{}", usage());
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunSummary {
    name: String,
    target_dir: PathBuf,
    files: Vec<PathBuf>,
    preset: Preset,
}

#[derive(Debug)]
enum CliError {
    Usage(&'static str),
    UnknownPreset(String),
    MissingValue(&'static str),
    UnexpectedArgument(String),
    CurrentDir(String),
    Spec(SpecError),
    Scaffold(ScaffoldError),
    Write(WriteError),
}

fn run<I, S>(args: I) -> Result<RunSummary, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);

    let command = args
        .next()
        .ok_or(CliError::Usage("missing command `new`"))?;
    if command == "--help" || command == "-h" {
        return Err(CliError::Usage("help requested"));
    }
    if command != "new" {
        return Err(CliError::Usage("expected command `new`"));
    }

    let preset_arg = args
        .next()
        .ok_or(CliError::Usage("missing preset `rust-cli`"))?;
    let preset = Preset::parse(&preset_arg).ok_or(CliError::UnknownPreset(preset_arg))?;

    let name = args.next().ok_or(CliError::Usage("missing project name"))?;
    let mut description = format!("{name} project.");
    let mut root = env::current_dir().map_err(|error| CliError::CurrentDir(error.to_string()))?;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--description" => {
                description = args.next().ok_or(CliError::MissingValue("--description"))?;
            }
            "--root" => {
                let value = args.next().ok_or(CliError::MissingValue("--root"))?;
                root = PathBuf::from(value);
            }
            "--help" | "-h" => return Err(CliError::Usage("help requested")),
            _ => return Err(CliError::UnexpectedArgument(arg)),
        }
    }

    let spec = ProjectSpec::new(name, description, preset, root).map_err(CliError::Spec)?;
    let files = generate(&spec).map_err(CliError::Scaffold)?;
    write_files(&files).map_err(CliError::Write)?;
    let target_dir = spec.target_dir();

    Ok(RunSummary {
        name: spec.name,
        target_dir,
        files: files.into_iter().map(|file| file.path).collect(),
        preset: spec.preset,
    })
}

fn print_summary(summary: &RunSummary) {
    println!(
        "created {} at {}",
        summary.name,
        summary.target_dir.display()
    );
    println!("files:");
    for path in &summary.files {
        println!("  - {}", display_path(path, &summary.target_dir));
    }
    println!("next steps:");
    if summary.preset == Preset::RepoSite {
        // Pages is a repository setting, not a file, so the preset cannot emit
        // it. Without this the first deploy fails on configure-pages with
        // "please verify that the repository has Pages enabled", which reads
        // like a broken workflow rather than a missing setting.
        println!("  - Enable Pages once (required before the workflow can run):");
        println!(
            "      gh api --method POST repos/89jobrien/{}/pages -f build_type=workflow",
            summary.name
        );
        println!("  - Fetch the fonts your tokens.css declares, then compose:");
        println!(
            "      bash <repo-reference-site>/scripts/fetch-fonts.sh <repo> \"family:weight\" ..."
        );
        println!(
            "      bash <repo-reference-site>/scripts/assemble-css.sh <repo> site/tokens.css site/signature.css"
        );
        println!("  - Validate: node <repo-reference-site>/scripts/validate-css.mjs <repo>");
    } else {
        println!("  - Review {}/CLAUDE.md", summary.name);
    }
    println!("  - Initialize git if this should be an independent repository");
    println!("  - Add a one-line description to the workspace project index if needed");
}

fn display_path(path: &Path, target_dir: &Path) -> String {
    path.strip_prefix(target_dir)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn usage() -> &'static str {
    "usage: sparkfile new rust-cli <name> [--description <text>] [--root <path>]"
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::UnknownPreset(preset) => write!(formatter, "unknown preset `{preset}`"),
            Self::MissingValue(flag) => write!(formatter, "missing value for `{flag}`"),
            Self::UnexpectedArgument(arg) => write!(formatter, "unexpected argument `{arg}`"),
            Self::CurrentDir(message) => {
                write!(formatter, "cannot read current directory: {message}")
            }
            Self::Spec(error) => error.fmt(formatter),
            Self::Scaffold(error) => error.fmt(formatter),
            Self::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir_name(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    // run() happy path — returns Ok(RunSummary) with expected name and files.
    #[test]
    fn run_happy_path() {
        let root = unique_dir_name("sparkfile-happy");
        let result = run([
            "new",
            "rust-cli",
            "my-tool",
            "--root",
            root.to_str().unwrap(),
        ]);
        // Clean up regardless of outcome.
        let _ = std::fs::remove_dir_all(&root);

        let summary = result.expect("run() should succeed");
        assert_eq!(summary.name, "my-tool");
        assert_eq!(summary.target_dir, root.join("my-tool"));
        assert!(
            !summary.files.is_empty(),
            "at least one file must be generated"
        );
        for path in &summary.files {
            assert!(
                path.starts_with(&summary.target_dir),
                "file {path:?} should be under target dir"
            );
        }
    }

    // run() with --help flag returns Err(CliError::Usage).
    #[test]
    fn run_help_flag() {
        let err = run(["--help"]).expect_err("--help must return an error");
        assert!(
            matches!(err, CliError::Usage(_)),
            "expected Usage, got {err:?}"
        );
    }

    // run() with an unknown preset returns Err(CliError::UnknownPreset).
    #[test]
    fn run_unknown_preset() {
        let err = run(["new", "no-such-preset", "my-tool"])
            .expect_err("unknown preset must return an error");
        assert!(
            matches!(err, CliError::UnknownPreset(ref s) if s == "no-such-preset"),
            "expected UnknownPreset(\"no-such-preset\"), got {err:?}"
        );
    }

    // run() with no name arg returns Err(CliError::Usage).
    #[test]
    fn run_missing_name() {
        let err = run(["new", "rust-cli"]).expect_err("missing name must return an error");
        assert!(
            matches!(err, CliError::Usage(_)),
            "expected Usage, got {err:?}"
        );
    }

    // run() with --description override reflects the override in the summary name
    // (the description field is not part of RunSummary, but the name must still be correct).
    #[test]
    fn run_description_override() {
        let root = unique_dir_name("sparkfile-desc");
        let result = run([
            "new",
            "rust-cli",
            "desc-tool",
            "--description",
            "A custom description.",
            "--root",
            root.to_str().unwrap(),
        ]);
        let _ = std::fs::remove_dir_all(&root);

        let summary = result.expect("run() with --description should succeed");
        assert_eq!(summary.name, "desc-tool");
    }

    // run() with --root pointing at a temp dir puts the target_dir under that root.
    #[test]
    fn run_root_override() {
        let root = unique_dir_name("sparkfile-root");
        let result = run([
            "new",
            "rust-cli",
            "rooted",
            "--root",
            root.to_str().unwrap(),
        ]);
        let _ = std::fs::remove_dir_all(&root);

        let summary = result.expect("run() with --root should succeed");
        assert_eq!(summary.target_dir, root.join("rooted"));
    }

    // display_path() strips the target_dir prefix when the path is inside it.
    #[test]
    fn display_path_strips_prefix() {
        let target = PathBuf::from("/tmp/workspace/my-tool");
        let file = target.join("src/main.rs");
        assert_eq!(display_path(&file, &target), "src/main.rs");
    }

    // display_path() falls back to the full path when the file is outside target_dir.
    #[test]
    fn display_path_fallback_outside_target() {
        let target = PathBuf::from("/tmp/workspace/my-tool");
        let outside = PathBuf::from("/tmp/other/file.txt");
        assert_eq!(display_path(&outside, &target), "/tmp/other/file.txt");
    }
}
