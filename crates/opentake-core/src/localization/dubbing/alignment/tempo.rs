//! Pitch-preserving tempo adjustment engine using FFmpeg atempo filter.

use std::path::Path;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::ai::tts::audio::validate_wav_file;
use crate::ai::tts::models::WavAudioMetadata;
use crate::media::discovery::{discover_tool, DiscoveryConfig, ToolType};
use crate::media::process::{run_managed_process, ProcessSpec};

use super::errors::DubbingAlignmentError;

/// Default timeout for audio tempo processing (30 seconds).
pub const DEFAULT_TEMPO_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Constructs the FFmpeg `atempo` filter expression.
/// FFmpeg atempo supports factors between 0.5 and 2.0 per filter instance.
/// Factors outside this range are chained sequentially (e.g. atempo=2.0,atempo=1.2).
pub fn build_atempo_filter(tempo_factor: f32) -> Result<String, DubbingAlignmentError> {
    if tempo_factor <= 0.0 {
        return Err(DubbingAlignmentError::TempoOutOfRange(
            tempo_factor,
            0.5,
            2.0,
        ));
    }

    // Clamp or format single / chained atempo stages
    if (0.5..=2.0).contains(&tempo_factor) {
        Ok(format!("atempo={tempo_factor:.4}"))
    } else if tempo_factor > 2.0 {
        let mut stages = Vec::new();
        let mut remaining = tempo_factor;
        while remaining > 2.0 {
            stages.push("atempo=2.0".to_string());
            remaining /= 2.0;
        }
        stages.push(format!("atempo={remaining:.4}"));
        Ok(stages.join(","))
    } else {
        // tempo_factor < 0.5
        let mut stages = Vec::new();
        let mut remaining = tempo_factor;
        while remaining < 0.5 {
            stages.push("atempo=0.5".to_string());
            remaining /= 0.5;
        }
        stages.push(format!("atempo={remaining:.4}"));
        Ok(stages.join(","))
    }
}

/// Builds explicit, secure FFmpeg argument vector for pitch-preserving tempo adjustment.
pub fn build_tempo_process_args(
    input_path: &Path,
    output_path: &Path,
    source_in_us: u64,
    source_out_us: u64,
    tempo_factor: f32,
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<String>, DubbingAlignmentError> {
    let mut args = vec!["-y".to_string()];

    // Fast seek to source trim in-point
    if source_in_us > 0 {
        let start_sec = (source_in_us as f64) / 1_000_000.0;
        args.push("-ss".to_string());
        args.push(format!("{start_sec:.6}"));
    }

    // Input path
    args.push("-i".to_string());
    args.push(input_path.to_string_lossy().to_string());

    // Duration limit to source trim out-point
    if source_out_us > source_in_us {
        let duration_sec = ((source_out_us - source_in_us) as f64) / 1_000_000.0;
        args.push("-t".to_string());
        args.push(format!("{duration_sec:.6}"));
    }

    // Audio filter: pitch-preserving atempo
    let atempo_filter = build_atempo_filter(tempo_factor)?;
    args.push("-af".to_string());
    args.push(atempo_filter);

    // Audio output specifications (uncompressed signed 16-bit PCM WAV)
    args.extend_from_slice(&[
        "-vn".to_string(),
        "-c:a".to_string(),
        "pcm_s16le".to_string(),
        "-ar".to_string(),
        sample_rate.to_string(),
        "-ac".to_string(),
        channels.to_string(),
        output_path.to_string_lossy().to_string(),
    ]);

    Ok(args)
}

/// Executes pitch-preserving tempo adjustment using system FFmpeg.
#[allow(clippy::too_many_arguments)]
pub async fn process_audio_tempo(
    input_path: &Path,
    output_path: &Path,
    source_in_us: u64,
    source_out_us: u64,
    tempo_factor: f32,
    sample_rate: u32,
    channels: u16,
    cancel_token: CancellationToken,
) -> Result<WavAudioMetadata, DubbingAlignmentError> {
    if !input_path.exists() {
        return Err(DubbingAlignmentError::AudioFileNotFound(
            input_path.to_string_lossy().to_string(),
        ));
    }

    // Discover system FFmpeg
    let tool_report =
        discover_tool(ToolType::Ffmpeg, &DiscoveryConfig::default()).map_err(|e| {
            DubbingAlignmentError::FfmpegExecutionFailed(format!("FFmpeg discovery error: {e}"))
        })?;

    let ffmpeg_bin = tool_report.path;

    let args = build_tempo_process_args(
        input_path,
        output_path,
        source_in_us,
        source_out_us,
        tempo_factor,
        sample_rate,
        channels,
    )?;

    let spec = ProcessSpec {
        program: ffmpeg_bin,
        args,
        timeout: DEFAULT_TEMPO_PROCESS_TIMEOUT,
        working_dir: None,
        max_stdout_bytes: 1024 * 1024,
        max_stderr_bytes: 1024 * 1024,
    };

    let result = run_managed_process(spec, Some(&cancel_token))
        .await
        .map_err(|e| DubbingAlignmentError::FfmpegExecutionFailed(e.to_string()))?;

    if cancel_token.is_cancelled() {
        let _ = std::fs::remove_file(output_path);
        return Err(DubbingAlignmentError::Cancelled);
    }

    if result.exit_code != Some(0) {
        let _ = std::fs::remove_file(output_path);
        return Err(DubbingAlignmentError::FfmpegExecutionFailed(format!(
            "FFmpeg exited with error (code {:?}): {}",
            result.exit_code,
            result.stderr.trim()
        )));
    }

    // Validate generated output WAV
    let meta = validate_wav_file(output_path).map_err(|e| {
        let _ = std::fs::remove_file(output_path);
        DubbingAlignmentError::InvalidOutputWav(e.to_string())
    })?;

    Ok(meta)
}
