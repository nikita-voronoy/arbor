use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

const HASHES_FILE: &str = "hashes.bin";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FileHashes {
    hashes: HashMap<PathBuf, u64>,
}

impl FileHashes {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hash a file's contents and return whether it changed.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    pub fn check_file(&mut self, path: &Path) -> Result<FileStatus> {
        let contents =
            std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        let hash = xxh3_64(&contents);

        match self.hashes.get(path) {
            Some(&old_hash) if old_hash == hash => Ok(FileStatus::Unchanged),
            Some(_) => {
                self.hashes.insert(path.to_path_buf(), hash);
                Ok(FileStatus::Modified)
            }
            None => {
                self.hashes.insert(path.to_path_buf(), hash);
                Ok(FileStatus::New)
            }
        }
    }

    /// Remove a file from the hash index
    pub fn remove_file(&mut self, path: &Path) {
        self.hashes.remove(path);
    }

    /// Get all tracked files
    pub fn tracked_files(&self) -> impl Iterator<Item = &Path> {
        self.hashes.keys().map(std::path::PathBuf::as_path)
    }

    /// Save hashes to disk.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root.join(".arbor");
        std::fs::create_dir_all(&dir)?;
        let data = bincode::serialize(self)?;
        std::fs::write(dir.join(HASHES_FILE), data)?;
        Ok(())
    }

    /// Load hashes from disk.
    ///
    /// # Errors
    /// Returns an error if the hash file exists but cannot be read or deserialized.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(".arbor").join(HASHES_FILE);
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read(&path)?;
        let hashes: Self = bincode::deserialize(&data)?;
        Ok(hashes)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileStatus {
    New,
    Modified,
    Unchanged,
}
