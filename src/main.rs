use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

use sparkfile::{
    codemeta::Metadata,
    docs::Docs,
    domain::{Preset, ProjectSpec, SpecError},
    fs::{WriteError, write_files},
    help::capture_help,
    scaffold::{ScaffoldError, generate, preset_files},
    sitegen,
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
    /// Stale outputs this run deleted, so a refresh never hides what it removed.
    removed: Vec<PathBuf>,
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
    Site(String),
}

fn run<I, S>(args: I) -> Result<RunSummary, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);

    let command = args
        .next()
        .ok_or(CliError::Usage("missing command `new` or `site`"))?;
    if command == "--help" || command == "-h" {
        return Err(CliError::Usage("help requested"));
    }
    if command == "site" {
        return generate_site(args.collect());
    }
    if command != "new" {
        return Err(CliError::Usage("expected command `new` or `site`"));
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
        removed: Vec::new(),
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
    if !summary.removed.is_empty() {
        println!("removed:");
        for path in &summary.removed {
            println!("  - {}", display_path(path, &summary.target_dir));
        }
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

/// `sparkfile site <repo> [--description <text>]`
///
/// Rewrites the body of an existing site from the repository's own
/// `cargo metadata`. Page shapes come from [`sparkfile::sitegen`]; this
/// writes them, composing a full HTML document around each body so the
/// pages carry the same nav and metadata a hand-written site would.
fn generate_site(args: Vec<String>) -> Result<RunSummary, CliError> {
    let repo = args.first().cloned().ok_or(CliError::Usage(
        "usage: sparkfile site <repo> [--description <text>] [--with-help]",
    ))?;
    let mut description = String::new();
    let mut with_help = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--description" => {
                description = args
                    .get(i + 1)
                    .cloned()
                    .ok_or(CliError::MissingValue("--description"))?;
                i += 2;
            }
            // Opt-in because each capture builds the binary, which is slow
            // enough that a site refresh should ask for it deliberately.
            "--with-help" => {
                with_help = true;
                i += 1;
            }
            other => return Err(CliError::UnexpectedArgument(other.to_string())),
        }
    }

    let root = Path::new(&repo);
    if !root.is_dir() {
        return Err(CliError::Site(format!("{repo} is not a directory")));
    }
    let metadata = Metadata::load(root).map_err(|e| CliError::Site(e.to_string()))?;
    let docs = Docs::load(root);

    let name = metadata
        .packages
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| repo.clone());
    if description.is_empty() {
        // README lede first, then the manifest summary: the lede explains the
        // project, the manifest field is a crates.io blurb.
        description = docs
            .lede
            .clone()
            .or_else(|| metadata.summary().map(str::to_string))
            .unwrap_or_else(|| format!("{name}."));
    }

    let mut help = Vec::new();
    if with_help {
        for package in &metadata.packages {
            for binary in package.binaries() {
                help.extend(capture_help(root, &binary.name));
            }
        }
    }

    let plan = sitegen::plan_with_docs(&name, &description, &metadata, &docs, &help);
    let site_dir = root.join("site");
    let mut written = Vec::new();
    let mut removed = Vec::new();
    for page in &plan.pages {
        let document = render_page(page, &plan, &description);
        let path = site_dir.join(page.output_path());
        // A subpage lives in its own directory, so the parent has to exist
        // before the file can be written into it.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Site(format!("cannot create site/: {e}")))?;
        }
        std::fs::write(&path, document)
            .map_err(|e| CliError::Site(format!("cannot write {}: {e}", path.display())))?;
        written.push(path);

        // Pages used to be written flat as `<slug>.html`. Removing exactly the
        // path this generator used to own for this slug keeps a site from
        // serving the same page at two URLs, which is what leaving them behind
        // does. Nothing else in site/ is touched.
        let legacy = site_dir.join(format!("{}.html", page.slug));
        if page.slug != "index" && legacy.is_file() {
            std::fs::remove_file(&legacy)
                .map_err(|e| CliError::Site(format!("cannot remove {}: {e}", legacy.display())))?;
            removed.push(legacy);
        }
    }

    // Commit the diagram source the architecture page points at. The renders
    // themselves come from render-diagrams.sh, which the next-steps text
    // points at; producing them here would duplicate that script's job.
    if let Some(diagram) = plan.pages.iter().find_map(|p| p.diagram.as_deref()) {
        let diagrams = site_dir.join("diagrams");
        std::fs::create_dir_all(&diagrams)
            .map_err(|e| CliError::Site(format!("cannot create site/diagrams: {e}")))?;
        let path = diagrams.join("architecture.mmd");
        std::fs::write(&path, diagram)
            .map_err(|e| CliError::Site(format!("cannot write {}: {e}", path.display())))?;
        written.push(path);
    }

    // The preset is the base layer: it owns tokens, the signature stub, the
    // diagram themes, and the workflow. Write only what is missing, so a
    // palette or theme someone has already tuned is never clobbered by
    // regenerating pages. This is also how a site generated before the
    // preset carried diagram themes picks them up.
    for (rel, contents) in preset_files(Preset::RepoSite).map_err(CliError::Scaffold)? {
        let path = root.join(&rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Site(format!("cannot create {}: {e}", parent.display())))?;
        }
        let filled = contents
            .replace("{{name}}", &name)
            .replace("{{description}}", &description);
        std::fs::write(&path, filled)
            .map_err(|e| CliError::Site(format!("cannot write {}: {e}", path.display())))?;
        written.push(path);
    }

    Ok(RunSummary {
        name,
        target_dir: root.to_path_buf(),
        files: written,
        removed,
        preset: Preset::RepoSite,
    })
}

/// Wrap a generated page body in the shared site's document chrome.
fn render_page(page: &sitegen::Page, plan: &sitegen::SitePlan, description: &str) -> String {
    // Every reference is relative to the page making it, so a subpage climbs
    // out of its own directory and the landing page, which is at the site
    // root, needs no prefix at all.
    let nav: String = plan
        .pages
        .iter()
        .map(|target| {
            let current = if target.slug == page.slug {
                r#" aria-current="page""#
            } else {
                ""
            };
            format!(
                "      <a href=\"{}\"{current}>{}</a>\n",
                page.href_to(target),
                target.nav_label
            )
        })
        .collect();

    // r## not r# : the skip-link href contains `"#`, which would close an
    // r"..." raw string at `href="#main"`.
    format!(
        r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="description" content="{description}">
    <title>{title}</title>
    <link rel="stylesheet" href="{asset}style.css">
  </head>
  <body>
    <a class="skip-link" href="#main">Skip to content</a>
    <nav class="nav" aria-label="Primary">
      <a href="{home}" class="brand">{brand}</a>
{nav}    </nav>

    <main id="main">
{body}    </main>
  </body>
</html>
"##,
        description = html_escape(description),
        title = html_escape(&page.title),
        brand = html_escape(&plan.project),
        asset = page.asset_prefix(),
        home = page.href_to(plan.landing()),
        nav = nav,
        body = page.body,
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
            Self::Site(message) => write!(formatter, "{message}"),
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
