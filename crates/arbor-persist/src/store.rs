use anyhow::{Context, Result};
use arbor_core::palace::Palace;
use std::path::Path;

const INDEX_DIR: &str = ".arbor";
const INDEX_FILE: &str = "index.bin";

/// Save the palace graph to disk
pub fn save(palace: &Palace, project_root: &Path) -> Result<()> {
    let dir = project_root.join(INDEX_DIR);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join(INDEX_FILE);
    let data = bincode::serialize(palace).context("Failed to serialize palace")?;
    std::fs::write(&path, data).context("Failed to write index")?;

    Ok(())
}

/// Load the palace graph from disk
pub fn load(project_root: &Path) -> Result<Option<Palace>> {
    let path = project_root.join(INDEX_DIR).join(INDEX_FILE);
    if !path.exists() {
        return Ok(None);
    }

    let data = std::fs::read(&path).context("Failed to read index")?;
    let palace: Palace = bincode::deserialize(&data).context("Failed to deserialize palace")?;
    Ok(Some(palace))
}
