//! Generate reference-site pages from `cargo metadata` and the README.
//!
//! The page set mirrors a hand-written pilot site: an overview, a guide
//! carried over from the README, an architecture page with the dependency
//! diagram, a crate inventory, a feature table, and a commands page when the
//! workspace ships binaries.
//!
//! Two sources, deliberately split by what each is good at. `cargo metadata`
//! is authoritative for structure — counts, versions, features, targets — and
//! nothing here invents one. The README is authoritative for intent, so the
//! lede and the guide are moved out of it rather than rewritten. Between them
//! the site can carry a sentence explaining why the project exists without
//! anyone maintaining that sentence in two places.

use crate::codemeta::Metadata;
use crate::docs::{Docs, body_to_html};
use crate::help::HelpCapture;

/// Re-exported so the CLI page renders help output exactly as a README's shell
/// examples render. One component, two sources.
use crate::docs::terminal_html_with_label;

/// One generated page: a slug, a title, and its HTML body.
///
/// `slug` is identity only — "guide", "crates". Where the page is written and
/// how other pages link to it are derived from it, so the layout is a property
/// of the generator rather than something repeated in every page constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub slug: String,
    pub title: String,
    pub nav_label: String,
    pub body: String,
    /// Mermaid source this page references, when it renders a diagram. The
    /// caller writes it so the `.mmd` is committed alongside the page rather
    /// than only existing in memory.
    pub diagram: Option<String>,
}

/// Prefix that reaches the site root from a page with this slug.
///
/// Assets live at the site root, so a subpage has to climb out of its own
/// directory to reach them and the landing page does not.
fn asset_prefix_for(slug: &str) -> &'static str {
    if slug == "index" { "" } else { "../" }
}

impl Page {
    /// The landing page is the one page that stays at the site root. Every
    /// other page becomes `<slug>/index.html`, so the URLs read `/guide/`
    /// rather than `/guide.html`, while the landing URL other sites link to
    /// never moves.
    pub fn is_landing(&self) -> bool {
        self.slug == "index"
    }

    /// Path this page is written to, relative to the site directory.
    pub fn output_path(&self) -> String {
        if self.is_landing() {
            "index.html".to_string()
        } else {
            format!("{}/index.html", self.slug)
        }
    }

    /// Prefix that reaches the site root from this page.
    pub fn asset_prefix(&self) -> &'static str {
        asset_prefix_for(&self.slug)
    }

    /// Clean URL linking this page to another.
    pub fn href_to(&self, other: &Page) -> String {
        let up = self.asset_prefix();
        if other.is_landing() {
            return if up.is_empty() {
                "./".to_string()
            } else {
                up.to_string()
            };
        }
        format!("{up}{}/", other.slug)
    }
}

/// The pages to generate, in nav order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePlan {
    pub project: String,
    pub description: String,
    pub pages: Vec<Page>,
}

impl SitePlan {
    /// The landing page, which every other page links back to.
    pub fn landing(&self) -> &Page {
        self.pages
            .iter()
            .find(|p| p.is_landing())
            .expect("every plan has a landing page")
    }

    /// Nav labels in page order, paired with the slug they point at.
    pub fn nav(&self) -> Vec<(&str, &str)> {
        self.pages
            .iter()
            .map(|p| (p.nav_label.as_str(), p.slug.as_str()))
            .collect()
    }
}

/// Build the page set for a workspace.
pub fn plan(project: &str, description: &str, metadata: &Metadata) -> SitePlan {
    plan_with_docs(project, description, metadata, &Docs::default(), &[])
}

/// Build the page set, taking prose from `docs` and CLI surface from `help`.
///
/// The README lede wins over the manifest description: Cargo's `description`
/// field is a crate-index summary, and a site hero is not a crate index.
pub fn plan_with_docs(
    project: &str,
    description: &str,
    metadata: &Metadata,
    docs: &Docs,
    help: &[HelpCapture],
) -> SitePlan {
    let lede = docs.lede.as_deref().unwrap_or(description).to_string();

    let mut pages = vec![overview_page(project, &lede, metadata)];

    if let Some(page) = guide_page(project, docs) {
        pages.push(page);
    }
    if let Some(diagram) = metadata.dependency_diagram() {
        pages.push(architecture_page(project, &diagram));
    }
    pages.push(crates_page(project, metadata));
    if metadata.total_features() > 0 {
        pages.push(features_page(project, metadata));
    }
    if !help.is_empty() {
        pages.push(cli_page(project, help));
    }
    if metadata.total_binaries() > 0 {
        pages.push(commands_page(project, metadata));
    }

    SitePlan {
        project: project.to_string(),
        description: lede,
        pages,
    }
}

/// The README's `##` sections, carried over as a guide.
fn guide_page(project: &str, docs: &Docs) -> Option<Page> {
    if docs.sections.is_empty() {
        return None;
    }

    let mut body = String::from(
        r#"      <section id="guide" class="reveal">
        <h2>Guide</h2>
        <p class="note">
          Carried over from <code>README.md</code>. Edit the README, not this
          page.
        </p>
"#,
    );
    for section in &docs.sections {
        body.push_str(&format!(
            "        <h3 id=\"{}\">{}</h3>\n{}\n",
            anchor(&section.title),
            escape(&section.title),
            body_to_html(&section.body)
        ));
    }
    if !docs.docs_dir.is_empty() {
        body.push_str("        <h3 id=\"further-reading\">Further reading</h3>\n        <ul>\n");
        for file in &docs.docs_dir {
            body.push_str(&format!(
                "          <li><code>docs/{}</code></li>\n",
                escape(file)
            ));
        }
        body.push_str("        </ul>\n");
    }
    body.push_str("      </section>\n");

    Some(Page {
        slug: "guide".into(),
        title: format!("{project} guide"),
        nav_label: "Guide".into(),
        body,
        diagram: None,
    })
}

/// One terminal per captured `--help`, the way crux documents its CLI.
fn cli_page(project: &str, help: &[HelpCapture]) -> Page {
    let mut body = String::from(
        r#"      <section id="cli" class="reveal">
        <h2>CLI</h2>
        <p class="note">
          Captured by running each binary with <code>--help</code>. It cannot
          drift from the tool the way a transcribed page can.
        </p>
"#,
    );
    for capture in help {
        body.push_str(&format!(
            "        <h3 id=\"{}\">{}</h3>\n{}",
            anchor(&capture.label),
            escape(&capture.label),
            terminal_html_with_label(&capture.output, &capture.label)
        ));
    }
    body.push_str("      </section>\n");

    Page {
        slug: "cli".into(),
        title: format!("{project} CLI"),
        nav_label: "CLI".into(),
        body,
        diagram: None,
    }
}

/// A heading title as an HTML id.
fn anchor(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn stat(value: &str, label: &str) -> String {
    format!(
        "          <div class=\"stat\">\n            \
         <span class=\"n\">{value}</span>\n            \
         <span class=\"d\">{label}</span>\n          </div>\n"
    )
}

fn overview_page(project: &str, description: &str, m: &Metadata) -> Page {
    let mut stats = String::new();
    stats.push_str(&stat(&m.packages.len().to_string(), "workspace crates"));
    stats.push_str(&stat(&m.total_binaries().to_string(), "binaries"));
    stats.push_str(&stat(
        &m.packages
            .iter()
            .map(|p| p.libraries().len())
            .sum::<usize>()
            .to_string(),
        "libraries",
    ));
    stats.push_str(&stat(&m.total_features().to_string(), "cargo features"));

    let mut rows = String::new();
    for pkg in &m.packages {
        let targets: Vec<String> = pkg
            .binaries()
            .iter()
            .map(|b| format!("`{}`", b.name))
            .chain(pkg.libraries().iter().map(|l| format!("lib `{}`", l.name)))
            .collect();
        rows.push_str(&format!(
            "              <tr>\n                <td><code>{}</code></td>\n                \
             <td>{}</td>\n                <td>{}</td>\n              </tr>\n",
            pkg.name,
            pkg.version,
            if targets.is_empty() {
                "&mdash;".to_string()
            } else {
                targets.join(", ")
            }
        ));
    }

    let body = format!(
        r#"      <section id="overview" class="reveal">
        <h2>Overview</h2>
        <p>{description}</p>
        <div class="stat-strip">
{stats}        </div>
        <div class="table-wrap">
          <table>
            <thead>
              <tr><th>Crate</th><th>Version</th><th>Targets</th></tr>
            </thead>
            <tbody>
{rows}            </tbody>
          </table>
        </div>
        <p class="note">
          Every figure on this site is read from <code>cargo metadata</code> at
          build time, so it cannot drift from the workspace.
        </p>
      </section>
"#,
        description = escape(description),
        stats = stats,
        rows = rows,
    );

    Page {
        slug: "index".into(),
        title: project.into(),
        nav_label: "Overview".into(),
        body,
        diagram: None,
    }
}

fn architecture_page(project: &str, diagram: &str) -> Page {
    // Matches the pilot sites: a committed .mmd source rendered to
    // screen + print SVG, wrapped in <picture>. An inline <pre class="mermaid">
    // would need a client-side Mermaid runtime the shared layer deliberately
    // does not carry, and an unstyled class.
    //
    // The architecture page is always a subpage, so the rendered SVGs live one
    // directory up and the reference has to climb out to reach them.
    let up = asset_prefix_for("architecture");
    let body = format!(
        r#"      <section id="architecture" class="reveal">
        <h2>Architecture</h2>
        <p>
          Internal dependencies only. External crates are omitted on purpose:
          a diagram meant to explain how {project} is put together should not be
          mostly tokio and serde.
        </p>
        <figure class="diagram">
          <picture>
            <source srcset="{up}diagrams/architecture.print.svg" media="print">
            <img src="{up}diagrams/architecture.svg" alt="Workspace crates, their binaries, and the dependencies between them.">
          </picture>
          <figcaption>
            Workspace crates and their binaries. Edit
            <code>site/diagrams/architecture.mmd</code>, not the rendered SVG.
          </figcaption>
        </figure>
      </section>
"#,
        project = escape(project),
    );

    Page {
        slug: "architecture".into(),
        title: format!("{project} architecture"),
        nav_label: "Architecture".into(),
        body,
        // Carried so the caller can write the .mmd source the figure points at.
        diagram: Some(diagram.to_string()),
    }
}

fn crates_page(project: &str, m: &Metadata) -> Page {
    let mut rows = String::new();
    for pkg in &m.packages {
        let deps: Vec<String> = pkg
            .internal_dependencies(m)
            .iter()
            .map(|d| format!("<code>{}</code>", d))
            .collect();
        let externals = pkg
            .dependencies
            .iter()
            .filter(|d| !m.packages.iter().any(|p| p.name == d.name))
            .count();
        rows.push_str(&format!(
            "              <tr>\n                <td><code>{}</code></td>\n                \
             <td>{}</td>\n                <td>{}</td>\n                <td>{}</td>\n              </tr>\n",
            pkg.name,
            pkg.version,
            if deps.is_empty() {
                "&mdash;".into()
            } else {
                deps.join(", ")
            },
            externals,
        ));
    }

    let body = format!(
        r#"      <section id="crates" class="reveal">
        <h2>Crates</h2>
        <p>Every package in the workspace, with what it depends on inside it.</p>
        <div class="table-wrap">
          <table>
            <thead>
              <tr><th>Crate</th><th>Version</th><th>Workspace deps</th><th>External deps</th></tr>
            </thead>
            <tbody>
{rows}            </tbody>
          </table>
        </div>
      </section>
"#,
        rows = rows,
    );

    Page {
        slug: "crates".into(),
        title: format!("{project} crates"),
        nav_label: "Crates".into(),
        body,
        diagram: None,
    }
}

fn features_page(project: &str, m: &Metadata) -> Page {
    let mut rows = String::new();
    for pkg in &m.packages {
        for (name, enables) in &pkg.features {
            rows.push_str(&format!(
                "              <tr>\n                <td><code>{}</code></td>\n                \
                 <td><code>{}</code></td>\n                <td>{}</td>\n              </tr>\n",
                name,
                pkg.name,
                if enables.is_empty() {
                    "&mdash;".into()
                } else {
                    escape(&enables.join(", "))
                }
            ));
        }
    }

    let body = format!(
        r#"      <section id="features" class="reveal">
        <h2>Features</h2>
        <p>
          Cargo features as declared, not as intended. A feature listed here
          with no enables clause is a flag nothing currently reads.
        </p>
        <div class="table-wrap">
          <table>
            <thead>
              <tr><th>Feature</th><th>Crate</th><th>Enables</th></tr>
            </thead>
            <tbody>
{rows}            </tbody>
          </table>
        </div>
      </section>
"#,
        rows = rows,
    );

    Page {
        slug: "features".into(),
        title: format!("{project} features"),
        nav_label: "Features".into(),
        body,
        diagram: None,
    }
}

fn commands_page(project: &str, m: &Metadata) -> Page {
    let mut rows = String::new();
    for pkg in &m.packages {
        for bin in pkg.binaries() {
            rows.push_str(&format!(
                "              <tr>\n                <td><code>{}</code></td>\n                \
                 <td><code>{}</code></td>\n                <td>{}</td>\n              </tr>\n",
                bin.name,
                pkg.name,
                bin.src_path
                    .rsplit('/')
                    .next()
                    .map(|f| format!("<code>{}</code>", f))
                    .unwrap_or_default()
            ));
        }
    }

    let body = format!(
        r#"      <section id="commands" class="reveal">
        <h2>Commands</h2>
        <p>Binaries the workspace ships, and the source file behind each.</p>
        <div class="table-wrap">
          <table>
            <thead>
              <tr><th>Command</th><th>Crate</th><th>Entry point</th></tr>
            </thead>
            <tbody>
{rows}            </tbody>
          </table>
        </div>
        <p class="note">
          Subcommands and flags are not listed because they are not in
          <code>cargo metadata</code>. Add them by hand, or generate them from
          <code>--help</code>, if this site needs to document them.
        </p>
      </section>
"#,
        rows = rows,
    );

    Page {
        slug: "commands".into(),
        title: format!("{project} commands"),
        nav_label: "Commands".into(),
        body,
        diagram: None,
    }
}

/// Escape for HTML text and attribute content.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Metadata {
        serde_json::from_str(
            r#"{
              "packages": [
                {"name":"core-lib","version":"0.3.0","description":"The core",
                 "features":{"fast":[],"extra":["dep:serde"]},
                 "dependencies":[{"name":"cli-bin","path":"../cli"},
                                 {"name":"serde","version":"1"}],
                 "targets":[{"name":"core-lib","kind":["lib"],"src_path":"src/lib.rs"},
                            {"name":"helper","kind":["bin"],"src_path":"src/bin/helper.rs"}]},
                {"name":"cli-bin","version":"0.3.0",
                 "dependencies":[{"name":"serde","version":"1"}],
                 "targets":[{"name":"cli","kind":["bin"],"src_path":"src/main.rs"}]}
              ],
              "workspace_members":["core-lib 0.3.0","cli-bin 0.3.0"],
              "workspace_root":"/repo"
            }"#,
        )
        .expect("parses")
    }

    #[test]
    fn plan_emits_overview_crates_and_commands() {
        let plan = plan("demo", "A demo.", &sample());
        let names: Vec<&str> = plan.pages.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(
            names,
            vec!["index", "architecture", "crates", "features", "commands"]
        );
    }

    #[test]
    fn overview_carries_real_counts_and_versions() {
        let plan = plan("demo", "A demo.", &sample());
        let index = &plan.pages[0].body;
        assert!(index.contains("workspace crates"));
        assert!(index.contains("core-lib"), "{index}");
        assert!(index.contains("0.3.0"));
        // counts: 2 crates, 2 binaries, 2 libs, 2 features
        assert!(index.contains(">2</span>"), "{index}");
    }

    #[test]
    fn crates_page_separates_internal_from_external_counts() {
        let plan = plan("demo", "A demo.", &sample());
        let crates = plan
            .pages
            .iter()
            .find(|p| p.slug == "crates")
            .expect("crates page");
        assert!(crates.body.contains("cli-bin"));
        assert!(crates.body.contains("Workspace deps"));
        // core-lib has one internal dep and one external
        assert!(
            crates.body.contains("<td>1</td>"),
            "{crates_body}",
            crates_body = crates.body
        );
    }

    #[test]
    fn features_page_lists_declared_features() {
        let plan = plan("demo", "A demo.", &sample());
        let features = plan
            .pages
            .iter()
            .find(|p| p.slug == "features")
            .expect("features page");
        assert!(features.body.contains("fast"));
        assert!(features.body.contains("dep:serde"));
    }

    #[test]
    fn pages_are_absent_when_they_would_be_empty() {
        let solo: Metadata = serde_json::from_str(
            r#"{"packages":[{"name":"solo","version":"0.1.0",
                 "dependencies":[{"name":"serde","version":"1"}],
                 "targets":[{"name":"solo","kind":["lib"],"src_path":"src/lib.rs"}]}]}"#,
        )
        .expect("parses");
        let plan = plan("solo", "A library.", &solo);
        let names: Vec<&str> = plan.pages.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(
            names,
            vec!["index", "crates"],
            "no binaries, features, or internal edges means no such pages"
        );
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn nav_matches_page_order() {
        let plan = plan("demo", "A demo.", &sample());
        let nav = plan.nav();
        assert_eq!(nav[0], ("Overview", "index"));
        assert_eq!(nav[1], ("Architecture", "architecture"));
        assert_eq!(nav.len(), plan.pages.len());
    }

    #[test]
    fn landing_page_stays_at_the_root_and_others_become_subpages() {
        let plan = plan("demo", "A demo.", &sample());
        let paths: Vec<String> = plan.pages.iter().map(|p| p.output_path()).collect();
        assert_eq!(
            paths,
            vec![
                "index.html",
                "architecture/index.html",
                "crates/index.html",
                "features/index.html",
                "commands/index.html",
            ],
            "the landing URL other sites link to must not move"
        );
    }

    #[test]
    fn links_are_clean_urls_relative_to_the_linking_page() {
        let plan = plan("demo", "A demo.", &sample());
        let landing = plan.landing();
        let crates = plan.pages.iter().find(|p| p.slug == "crates").unwrap();

        // From the landing page, siblings need no prefix.
        assert_eq!(landing.href_to(crates), "crates/");
        assert_eq!(landing.href_to(landing), "./");
        assert_eq!(landing.asset_prefix(), "");

        // From a subpage, everything climbs out first.
        assert_eq!(crates.href_to(landing), "../");
        assert_eq!(
            crates.href_to(
                plan.pages
                    .iter()
                    .find(|p| p.slug == "architecture")
                    .unwrap()
            ),
            "../architecture/"
        );
        assert_eq!(crates.asset_prefix(), "../");
    }

    #[test]
    fn architecture_diagram_climbs_to_the_site_root() {
        let plan = plan("demo", "A demo.", &sample());
        let architecture = plan
            .pages
            .iter()
            .find(|p| p.slug == "architecture")
            .expect("architecture page");
        assert!(
            architecture
                .body
                .contains(r#"srcset="../diagrams/architecture.print.svg""#)
        );
        assert!(
            architecture
                .body
                .contains(r#"src="../diagrams/architecture.svg""#)
        );
    }
}
