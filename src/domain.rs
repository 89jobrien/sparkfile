use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    RustCli,
    /// A GitHub Pages reference site: index, tokens, signature, workflow.
    RepoSite,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSpec {
    pub name: String,
    pub description: String,
    pub preset: Preset,
    pub root: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    InvalidName(String),
}

impl Preset {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rust-cli" => Some(Self::RustCli),
            "repo-site" => Some(Self::RepoSite),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustCli => "rust-cli",
            Self::RepoSite => "repo-site",
        }
    }
}

impl ProjectSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        preset: Preset,
        root: impl Into<PathBuf>,
    ) -> Result<Self, SpecError> {
        let name = name.into();
        validate_name(&name)?;
        Ok(Self {
            name,
            description: description.into(),
            preset,
            root: root.into(),
        })
    }

    pub fn target_dir(&self) -> PathBuf {
        self.root.join(&self.name)
    }
}

fn validate_name(name: &str) -> Result<(), SpecError> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(SpecError::InvalidName(name.to_string()));
    };

    if !first.is_ascii_lowercase() {
        return Err(SpecError::InvalidName(name.to_string()));
    }

    if chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
        Ok(())
    } else {
        Err(SpecError::InvalidName(name.to_string()))
    }
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(name) => write!(
                formatter,
                "invalid project name `{name}`; use lowercase letters, digits, hyphens, or underscores, starting with a lowercase letter"
            ),
        }
    }
}

impl std::error::Error for SpecError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_project_names_with_digits_hyphens_and_underscores() {
        let spec = ProjectSpec::new(
            "my_tool-2",
            "test project",
            Preset::RustCli,
            "/tmp/workspace",
        )
        .expect("valid project name");

        assert_eq!(spec.name, "my_tool-2");
        assert_eq!(spec.target_dir(), PathBuf::from("/tmp/workspace/my_tool-2"));
    }

    #[test]
    fn rejects_invalid_project_names() {
        for name in [
            "MyTool", "my tool", ".hidden", "tool!", "", "-tool", "_tool",
        ] {
            let error = ProjectSpec::new(name, "test project", Preset::RustCli, "/tmp/workspace")
                .expect_err("invalid project name should fail");

            assert_eq!(error, SpecError::InvalidName(name.to_string()));
        }
    }
}
