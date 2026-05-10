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
    println!("  - Review {}/CLAUDE.md", summary.name);
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
