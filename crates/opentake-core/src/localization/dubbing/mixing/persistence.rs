//! Mixing configuration persistence helpers for DubbingProject and SubtitleDocument.

use super::models::{DubbingMixConfiguration, DubbingMixResult};
use crate::localization::dubbing::models::DubbingProject;

/// Embeds or updates mixing configuration inside a DubbingProject.
pub fn set_project_mix_configuration(
    project: &mut DubbingProject,
    config: DubbingMixConfiguration,
) {
    project.mix_configuration = Some(config);
    project.touch();
}

/// Embeds or updates mixing result inside a DubbingProject.
pub fn set_project_mix_result(project: &mut DubbingProject, result: DubbingMixResult) {
    project.mix_result = Some(result);
    project.touch();
}

/// Clears previous mix result (marking mixdown as stale or invalid).
pub fn invalidate_project_mix_result(project: &mut DubbingProject) {
    project.mix_result = None;
    project.touch();
}
