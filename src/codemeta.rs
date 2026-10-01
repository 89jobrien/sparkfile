//! Derive site content from a crate's own `cargo metadata`.
//!
//! Everything this module produces is read from the build manifest rather
//! than written by hand, so a site cannot drift from the code it documents:
//! a crate is added, a feature is dropped, a binary is renamed, and the
//! corresponding page changes on the next run.
//!
//! The source is `cargo metadata --no-deps --format-version 1`, which is
//! Cargo's own machine-readable description of a workspace. It is used in
//! preference to parsing `Cargo.toml` text because it resolves inheritance,
//! workspace defaults, and target conditionals rather than reporting only
//! what is literally written in the file.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// The subset of `cargo metadata` this module needs.
///
/// Field names match Cargo's own JSON, so they stay snake_case; renaming one
/// would silently stop populating it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Metadata {
    #[serde(default)]
    pub packages: Vec<Package>,
    #[serde(default)]
    pub workspace_members: Vec<String>,
    #[serde(default)]
    pub workspace_root: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Package {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub manifest_path: String,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Target {
    pub name: String,
    #[serde(default)]
    pub kind: Vec<String>,
    #[serde(default)]
    pub src_path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Dependency {
    pub name: String,
    /// Absent for path and workspace dependencies, which is the distinction
    /// that matters for a dependency diagram.
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl Target {
    pub fn is_bin(&self) -> bool {
        self.kind.iter().any(|k| k == "bin")
    }

    pub fn is_lib(&self) -> bool {
        self.kind.iter().any(|k| k == "lib")
    }
}

impl Package {
    pub fn binaries(&self) -> Vec<&Target> {
        self.targets.iter().filter(|t| t.is_bin()).collect()
    }

    pub fn libraries(&self) -> Vec<&Target> {
        self.targets.iter().filter(|t| t.is_lib()).collect()
    }

    /// Internal dependencies, resolved against this workspace's packages.
    ///
    /// External dependencies are excluded on purpose: a dependency diagram
    /// meant to explain how a project is put together should not be 80%
    /// tokio and serde.
    pub fn internal_dependencies(&self, metadata: &Metadata) -> Vec<String> {
        self.dependencies
            .iter()
            .filter(|dep| metadata.packages.iter().any(|p| p.name == dep.name))
            .map(|dep| dep.name.clone())
            .collect()
    }
}

impl Metadata {
    /// Run `cargo metadata` in `root` and parse the result.
    pub fn load(root: &Path) -> Result<Self, SiteError> {
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(root)
            .output()
            .map_err(|source| SiteError::Cargo {
                reason: format!("could not run cargo: {source}"),
            })?;

        if !output.status.success() {
            return Err(SiteError::Cargo {
                reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        serde_json::from_slice(&output.stdout).map_err(|source| SiteError::Parse {
            reason: source.to_string(),
        })
    }

    /// A short description for the package, falling back to the workspace.
    pub fn summary(&self) -> Option<&str> {
        self.packages.iter().find_map(|p| p.description.as_deref())
    }

    pub fn total_binaries(&self) -> usize {
        self.packages
            .iter()
            .map(|p| p.binaries().len())
            .sum::<usize>()
    }

    pub fn total_features(&self) -> usize {
        self.packages.iter().map(|p| p.features.len()).sum()
    }

    /// A Mermaid flowchart of internal dependencies.
    ///
    /// Only edges between workspace members are drawn. Mermaid identifiers
    /// cannot contain `-`, which Cargo crate names routinely do, so each name
    /// is mapped to a safe id and the label carries the real name.
    pub fn dependency_diagram(&self) -> Option<String> {
        let has_internal_edge = self
            .packages
            .iter()
            .any(|p| !p.internal_dependencies(self).is_empty());
        if !has_internal_edge {
            return None;
        }

        let mut out = String::from("flowchart LR\n");
        for pkg in &self.packages {
            let id = mermaid_id(&pkg.name);
            out.push_str(&format!(
                "  {id}[\"{}<br/>{}\"]\n",
                escape(&pkg.name),
                escape(&pkg.version)
            ));
            let bins = pkg.binaries();
            if !bins.is_empty() {
                let names: Vec<String> = bins.iter().map(|b| escape(&b.name)).collect();
                out.push_str(&format!(
                    "  {}[\"{}\"]\n",
                    binary_node_id(&pkg.name),
                    escape(&names.join(", "))
                ));
            }
        }
        for pkg in &self.packages {
            let id = mermaid_id(&pkg.name);
            for dep in pkg.internal_dependencies(self) {
                out.push_str(&format!("  {id} --> {}\n", mermaid_id(&dep)));
            }
            for bin in pkg.binaries() {
                out.push_str(&format!("  {id} --> {}\n", binary_node_id(&bin.name)));
            }
        }
        Some(out)
    }
}

/// Mermaid node ids must match `[A-Za-z0-9_]`; crate names use hyphens.
fn mermaid_id(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            // `to_digit` returns None for punctuation, which would map
            // '-' '.' and '/' to the same sequence and let two different
            // crate names collide on one node id. The codepoint does not.
            out.push_str(&format!("x{:x}x", ch as u32));
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'n');
    }
    out
}

/// Node id for a binary, kept in a namespace of its own so a binary named
/// like a crate cannot land on the crate's node.
fn binary_node_id(bin_name: &str) -> String {
    format!("bin_{}", mermaid_id(bin_name))
}

/// Quotes and angle brackets would close a Mermaid label early.
fn escape(value: &str) -> String {
    value.replace('"', "'").replace(['<', '>'], "-")
}

#[derive(Debug)]
pub enum SiteError {
    Cargo { reason: String },
    Parse { reason: String },
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cargo { reason } => write!(f, "cargo metadata failed: {reason}"),
            Self::Parse { reason } => write!(f, "could not parse cargo metadata: {reason}"),
        }
    }
}

impl std::error::Error for SiteError {}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "packages": [
        {
          "name": "core-lib",
          "version": "0.3.0",
          "description": "The core",
          "features": { "extra": [], "fast": [] },
          "dependencies": [
            { "name": "cli-bin", "path": "../cli" },
            { "name": "serde", "version": "1" }
          ],
          "targets": [
            { "name": "core-lib", "kind": ["lib"], "src_path": "src/lib.rs" },
            { "name": "helper", "kind": ["bin"], "src_path": "src/bin/helper.rs" }
          ]
        },
        {
          "name": "cli-bin",
          "version": "0.3.0",
          "dependencies": [ { "name": "serde", "version": "1" } ],
          "targets": [ { "name": "cli", "kind": ["bin"], "src_path": "src/main.rs" } ]
        }
      ],
      "workspace_members": ["core-lib 0.3.0", "cli-bin 0.3.0"],
      "workspace_root": "/repo"
    }"#;

    fn sample() -> Metadata {
        serde_json::from_str(SAMPLE).expect("sample parses")
    }

    #[test]
    fn reads_packages_versions_and_targets() {
        let m = sample();
        assert_eq!(m.packages.len(), 2);
        assert_eq!(m.packages[0].version, "0.3.0");
        assert_eq!(m.packages[0].features.len(), 2);
        assert_eq!(m.packages[0].binaries().len(), 1);
        assert_eq!(m.packages[0].libraries().len(), 1);
        assert_eq!(m.total_binaries(), 2);
        assert_eq!(m.total_features(), 2);
    }

    #[test]
    fn internal_dependencies_exclude_external_crates() {
        let m = sample();
        assert_eq!(
            m.packages[0].internal_dependencies(&m),
            vec!["cli-bin".to_string()],
            "serde is external and must not appear"
        );
        assert!(m.packages[1].internal_dependencies(&m).is_empty());
    }

    #[test]
    fn dependency_diagram_draws_only_internal_edges() {
        let m = sample();
        let diagram = m.dependency_diagram().expect("has an internal edge");
        assert!(diagram.starts_with("flowchart LR"));
        assert!(
            diagram.contains(&format!(
                "{} --> {}",
                mermaid_id("core-lib"),
                mermaid_id("cli-bin")
            )),
            "{diagram}"
        );
        assert!(
            !diagram.contains("serde"),
            "external deps must not be drawn: {diagram}"
        );
    }

    #[test]
    fn diagram_is_none_when_everything_is_external() {
        let m: Metadata = serde_json::from_str(
            r#"{"packages":[{"name":"solo","version":"0.1.0",
                 "dependencies":[{"name":"serde","version":"1"}],
                 "targets":[]}]}"#,
        )
        .expect("parses");
        assert!(m.dependency_diagram().is_none());
    }

    #[test]
    fn hyphenated_names_become_valid_mermaid_ids() {
        let m: Metadata = serde_json::from_str(
            r#"{"packages":[
                 {"name":"minibox-core","version":"0.1.0","dependencies":[],
                  "targets":[]},
                 {"name":"minibox-cli","version":"0.1.0",
                  "dependencies":[{"name":"minibox-core","path":"../core"}],
                  "targets":[{"name":"minibox","kind":["bin"],"src_path":"x"}]}]}"#,
        )
        .expect("parses");

        let diagram = m.dependency_diagram().expect("internal edge");
        assert!(
            diagram.contains(&format!(
                "{} --> {}",
                mermaid_id("minibox-cli"),
                mermaid_id("minibox-core")
            )),
            "{diagram}"
        );
        // Every id used must be identifier-safe.
        for line in diagram.lines().skip(1) {
            let id = line.trim().split(['[', ' ']).next().unwrap_or("");
            assert!(
                id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "unsafe mermaid id {id:?} in {line:?}"
            );
        }
    }

    #[test]
    fn distinct_unsafe_characters_get_distinct_ids() {
        // `char::to_digit` returns None for punctuation, so an encoding built
        // on it collapses '-' and '.' onto the same id. These must differ or
        // two crates would silently merge into one node.
        assert_ne!(mermaid_id("a-b"), mermaid_id("a.b"));
        assert_ne!(mermaid_id("a-b"), mermaid_id("a/b"));
        assert_ne!(mermaid_id("a.b"), mermaid_id("a/b"));
    }

    #[test]
    fn a_binary_named_like_a_crate_gets_its_own_node() {
        let m: Metadata = serde_json::from_str(
            r#"{"packages":[
                 {"name":"helper","version":"0.1.0","dependencies":[],
                  "targets":[{"name":"helper","kind":["bin"],"src_path":"x"}]},
                 {"name":"caller","version":"0.1.0",
                  "dependencies":[{"name":"helper","path":"../helper"}],
                  "targets":[{"name":"caller","kind":["lib"],"src_path":"y"}]}
               ]}"#,
        )
        .expect("parses");
        let diagram = m.dependency_diagram().expect("has an internal edge");
        assert_ne!(
            binary_node_id("helper"),
            mermaid_id("helper"),
            "a binary and a crate of the same name must not share a node"
        );
        assert!(diagram.contains(&binary_node_id("helper")), "{diagram}");
    }

    #[test]
    fn labels_are_escaped() {
        assert_eq!(escape("a\"b"), "a'b");
        assert_eq!(escape("<br/>"), "-br/-");
    }
}
