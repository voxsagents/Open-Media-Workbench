//! Mixdown planner: resolves active tracks, segments, ducking envelopes, and builds DubbingMixPlan.

use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::localization::dubbing::models::DubbingProject;
use crate::media::discovery::{discover_tool, DiscoveryConfig, ToolType};
use crate::media::process::{run_managed_process, ProcessSpec};

use super::ducking::compute_ducking_intervals;
use super::errors::DubbingMixError;
use super::filters::build_complex_filter_graph;
use super::models::{linear_to_db, DubbingMixConfiguration, DubbingMixPlan, DubbingMixSegment};

/// Probes a media file with FFprobe to determine if it contains an active audio stream.
pub async fn probe_media_has_audio(media_path: &Path) -> (bool, u32, u16) {
    if !media_path.exists() {
        return (false, 48000, 2);
    }

    let Ok(discovered) = discover_tool(ToolType::Ffprobe, &DiscoveryConfig::default()) else {
        return (false, 48000, 2);
    };

    let spec = ProcessSpec::new(
        discovered.canonical_path,
        vec![
            "-v".to_string(),
            "error".to_string(),
            "-select_streams".to_string(),
            "a:0".to_string(),
            "-show_entries".to_string(),
            "stream=sample_rate,channels".to_string(),
            "-of".to_string(),
            "default=noprint_wrappers=1:nokey=1".to_string(),
            media_path.to_string_lossy().to_string(),
        ],
        std::time::Duration::from_secs(5),
    );

    let cancel = tokio_util::sync::CancellationToken::new();
    let Ok(output) = run_managed_process(spec, Some(&cancel)).await else {
        return (false, 48000, 2);
    };

    let lines: Vec<&str> = output
        .stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return (false, 48000, 2);
    }

    let sample_rate = lines
        .first()
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(48000);
    let channels = lines
        .get(1)
        .and_then(|l| l.parse::<u16>().ok())
        .unwrap_or(2);

    (true, sample_rate, channels)
}

/// Constructs an authoritative mixing plan from a DubbingProject and configuration.
pub async fn plan_dubbing_mix(
    project: &DubbingProject,
    config: &DubbingMixConfiguration,
    media_path_override: Option<&Path>,
) -> Result<DubbingMixPlan, DubbingMixError> {
    let mut warnings: Vec<String> = Vec::new();

    // 1. Probe original media soundtrack
    let resolved_media_path = media_path_override
        .map(PathBuf::from)
        .or_else(|| project.media_reference.as_ref().map(PathBuf::from));

    let (has_orig_stream, orig_sr, orig_ch) = if let Some(ref mpath) = resolved_media_path {
        if mpath.exists() {
            let (has_audio, sr, ch) = probe_media_has_audio(mpath).await;
            if !has_audio {
                warnings.push(format!(
                    "Source media '{}' does not contain an audio stream. Proceeding with speech-only mixdown.",
                    mpath.display()
                ));
            }
            (has_audio, sr, ch)
        } else {
            warnings.push(format!(
                "Source media path '{}' not found on disk. Proceeding with speech-only mixdown.",
                mpath.display()
            ));
            (false, 48000, 2)
        }
    } else {
        (false, 48000, 2)
    };

    let include_original = config.original_audio_enabled && has_orig_stream;

    // 2. Resolve solo and mute rules across tracks
    let any_track_soloed = project.tracks.iter().any(|t| {
        let t_settings = config.track_settings.get(&t.id);
        t_settings.map(|s| s.is_solo).unwrap_or(t.is_solo)
    });

    // 3. Resolve active speech segments
    let mut resolved_segments: Vec<DubbingMixSegment> = Vec::new();

    for track in &project.tracks {
        let t_settings = config.track_settings.get(&track.id);
        let track_enabled = t_settings.map(|s| s.is_enabled).unwrap_or(track.is_enabled);
        let track_muted = t_settings.map(|s| s.is_muted).unwrap_or(track.is_muted);
        let track_solo = t_settings.map(|s| s.is_solo).unwrap_or(track.is_solo);
        let track_gain = t_settings.map(|s| s.gain_db).unwrap_or(0.0);

        if !track_enabled || track_muted {
            continue;
        }

        if any_track_soloed && !track_solo {
            continue;
        }

        for seg in &track.segments {
            // Determine active asset
            let (asset_id, is_aligned) = if seg.is_aligned() {
                (seg.aligned_audio_asset_id.as_ref().unwrap().clone(), true)
            } else {
                (seg.audio_asset_id.clone(), false)
            };

            let Some(asset) = project.assets.get(&asset_id) else {
                warnings.push(format!(
                    "Segment '{}' references missing audio asset '{}'. Segment omitted from mix.",
                    seg.id, asset_id
                ));
                continue;
            };

            if !asset.file_path.exists() {
                warnings.push(format!(
                    "Audio asset file '{}' is missing on disk. Segment '{}' omitted from mix.",
                    asset.file_path.display(),
                    seg.id
                ));
                continue;
            }

            let dur_us = if is_aligned {
                asset.duration_us
            } else {
                seg.source_out_us.saturating_sub(seg.source_in_us)
            };

            if dur_us == 0 {
                continue;
            }

            let seg_gain_db = linear_to_db(seg.gain) + track_gain;

            resolved_segments.push(DubbingMixSegment {
                segment_id: seg.id.clone(),
                track_id: track.id.clone(),
                asset_id,
                resolved_file_path: asset.file_path.clone(),
                timeline_start_us: seg.timeline_start_us,
                duration_us: dur_us,
                source_in_us: if is_aligned { 0 } else { seg.source_in_us },
                source_out_us: if is_aligned {
                    asset.duration_us
                } else {
                    seg.source_out_us
                },
                gain_db: seg_gain_db,
                fade_in_ms: 0,
                fade_out_ms: 0,
                is_aligned,
            });
        }
    }

    // Sort segments chronologically
    resolved_segments.sort_by_key(|s| s.timeline_start_us);

    // Ensure we have at least one valid audio input
    if !include_original && resolved_segments.is_empty() {
        return Err(DubbingMixError::NoAudioInputs);
    }

    // 4. Compute dialogue ducking intervals
    let ducking_intervals = compute_ducking_intervals(&resolved_segments, config);

    // 5. Determine total timeline duration
    let max_seg_end_us = resolved_segments
        .iter()
        .map(|s| s.timeline_start_us + s.duration_us)
        .max()
        .unwrap_or(0);
    let timeline_duration_us = project
        .timeline_duration_us
        .max(max_seg_end_us)
        .max(1_000_000);

    let mut plan = DubbingMixPlan {
        plan_id: Uuid::new_v4().to_string(),
        project_id: project.id.clone(),
        timeline_duration_us,
        has_original_audio: include_original,
        original_audio_path: if include_original {
            resolved_media_path
        } else {
            None
        },
        original_audio_sample_rate: orig_sr,
        original_audio_channels: orig_ch,
        original_gain_db: config.original_audio_gain_db,
        dubbing_master_gain_db: config.dubbing_master_gain_db,
        segments: resolved_segments,
        ducking_intervals,
        output_sample_rate: config.output_sample_rate,
        output_channels: config.output_channels,
        filter_graph_desc: String::new(),
        warnings,
    };

    plan.filter_graph_desc = build_complex_filter_graph(&plan);

    Ok(plan)
}
