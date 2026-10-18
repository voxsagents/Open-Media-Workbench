//! Project persistence and durable storage for dubbing projects.

use std::fs;
use std::path::{Path, PathBuf};

use super::errors::DubbingError;
use super::models::DubbingProject;
use crate::localization::subtitles::model::SubtitleDocument;

/// Metadata key used to embed dubbing project state in SubtitleDocument.
pub const DUBBING_METADATA_KEY: &str = "dubbingProject";

/// Saves a DubbingProject into SubtitleDocument metadata and optionally writes to disk atomically.
pub fn save_dubbing_project_to_document(
    document: &mut SubtitleDocument,
    project: &DubbingProject,
) -> Result<(), DubbingError> {
    let serialized = serde_json::to_string(project)?;
    document
        .metadata
        .insert(DUBBING_METADATA_KEY.to_string(), serialized);
    document.touch();
    Ok(())
}

/// Loads a DubbingProject from SubtitleDocument metadata if present.
pub fn load_dubbing_project_from_document(
    document: &SubtitleDocument,
) -> Result<Option<DubbingProject>, DubbingError> {
    if let Some(raw_json) = document.metadata.get(DUBBING_METADATA_KEY) {
        let project: DubbingProject = serde_json::from_str(raw_json)?;
        Ok(Some(project))
    } else {
        Ok(None)
    }
}

/// Atomically writes a DubbingProject to a JSON file on disk.
pub fn save_dubbing_project_to_file(
    project: &DubbingProject,
    target_path: &Path,
) -> Result<(), DubbingError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = target_path.with_extension("tmp");
    let serialized = serde_json::to_string_pretty(project)?;

    fs::write(&temp_path, serialized)?;
    fs::rename(&temp_path, target_path)?;

    Ok(())
}

/// Reads a DubbingProject from a JSON file on disk.
pub fn load_dubbing_project_from_file(source_path: &Path) -> Result<DubbingProject, DubbingError> {
    let content = fs::read_to_string(source_path)?;
    let project: DubbingProject = serde_json::from_str(&content)?;
    Ok(project)
}

/// Returns the standard durable project directory for dubbing.
pub fn get_dubbing_project_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("dubbing").join("projects")
}
