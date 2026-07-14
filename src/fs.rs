use std::path::PathBuf;

use crate::scaffold::FileEntry;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    Conflict(PathBuf),
    Io(String),
}

pub fn write_files(files: &[FileEntry]) -> Result<(), WriteError> {
    for file in files {
        if file.path.exists() {
            return Err(WriteError::Conflict(file.path.clone()));
        }
    }

    for file in files {
        if let Some(parent) = file.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                WriteError::Io(format!("cannot create {}: {error}", parent.display()))
            })?;
        }

        std::fs::write(&file.path, &file.contents).map_err(|error| {
            WriteError::Io(format!("cannot write {}: {error}", file.path.display()))
        })?;
    }
    Ok(())
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict(path) => write!(formatter, "refusing to overwrite {}", path.display()),
            Self::Io(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WriteError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("sparkfile-test-{suffix}"))
    }

    #[test]
    fn writes_files_and_parent_directories() {
        let root = temp_root();
        let target = root.join("demo/src/main.rs");
        let files = vec![FileEntry {
            path: target.clone(),
            contents: "fn main() {}\n".to_string(),
        }];

        write_files(&files).expect("files should be written");

        assert_eq!(
            fs::read_to_string(&target).expect("generated file should exist"),
            "fn main() {}\n"
        );

        if root.exists() {
            fs::remove_dir_all(root).expect("cleanup temp root");
        }
    }

    #[test]
    fn refuses_to_overwrite_existing_files() {
        let root = temp_root();
        let target = root.join("demo/README.md");
        fs::create_dir_all(target.parent().unwrap()).expect("create parent");
        fs::write(&target, "existing\n").expect("seed existing file");

        let files = vec![FileEntry {
            path: target.clone(),
            contents: "new\n".to_string(),
        }];

        let error = write_files(&files).expect_err("existing file should conflict");

        assert_eq!(error, WriteError::Conflict(target));
        assert_eq!(
            fs::read_to_string(root.join(Path::new("demo/README.md")))
                .expect("existing file should remain"),
            "existing\n"
        );

        if root.exists() {
            fs::remove_dir_all(root).expect("cleanup temp root");
        }
    }
}
