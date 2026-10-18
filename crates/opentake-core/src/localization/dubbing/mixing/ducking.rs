//! Dialogue ducking engine and interval merge algorithms.

use super::models::{DubbingMixConfiguration, DubbingMixSegment, DuckingInterval};

/// Calculates non-overlapping, smoothed dialogue ducking intervals from active speech segments.
///
/// Ensures dialogue ducking starts early (attack pre-roll), holds through pauses,
/// releases smoothly, and merges adjacent windows to prevent volume pumping.
pub fn compute_ducking_intervals(
    segments: &[DubbingMixSegment],
    config: &DubbingMixConfiguration,
) -> Vec<DuckingInterval> {
    if !config.ducking_enabled || segments.is_empty() {
        return Vec::new();
    }

    let attack_us = (config.ducking_attack_ms as u64) * 1000;
    let release_us = (config.ducking_release_ms as u64) * 1000;
    let hold_us = (config.ducking_hold_ms as u64) * 1000;
    let attenuation_db = config.ducking_attenuation_db;

    // 1. Generate raw speech intervals
    let mut raw_spans: Vec<(u64, u64)> = segments
        .iter()
        .filter(|s| s.duration_us > 0)
        .map(|s| (s.timeline_start_us, s.timeline_start_us + s.duration_us))
        .collect();

    if raw_spans.is_empty() {
        return Vec::new();
    }

    // Sort chronologically by start time
    raw_spans.sort_by_key(|span| span.0);

    // 2. Merge speech spans separated by less than (hold + release + attack)
    // to prevent unnatural volume pumping between rapid dialogue utterances
    let merge_threshold_us = hold_us + release_us + attack_us;
    let mut merged_spans: Vec<(u64, u64)> = Vec::new();
    let mut current = raw_spans[0];

    for next in raw_spans.into_iter().skip(1) {
        if next.0 <= current.1 + merge_threshold_us {
            // Merge into current span
            current.1 = current.1.max(next.1);
        } else {
            merged_spans.push(current);
            current = next;
        }
    }
    merged_spans.push(current);

    // 3. Build DuckingInterval instances with attack, hold, and release envelopes
    merged_spans
        .into_iter()
        .map(|(speech_start, speech_end)| DuckingInterval {
            start_us: speech_start,
            end_us: speech_end + hold_us,
            attack_us,
            release_us,
            attenuation_db,
        })
        .collect()
}
