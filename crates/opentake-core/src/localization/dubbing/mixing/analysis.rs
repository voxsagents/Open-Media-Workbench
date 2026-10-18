//! Audio quality, loudness, peak, and clipping analysis for mixed WAV assets.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::errors::DubbingMixError;
use super::models::linear_to_db;

/// Measured acoustic metrics extracted from a PCM WAV file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAnalysisReport {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub duration_us: u64,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub integrated_lufs: Option<f32>,
    pub clipping_samples: u64,
}

/// Analyzes an uncompressed 16-bit PCM WAV file.
pub fn analyze_pcm_wav_file(wav_path: &Path) -> Result<AudioAnalysisReport, DubbingMixError> {
    if !wav_path.exists() {
        return Err(DubbingMixError::AssetNotFound(
            wav_path.to_string_lossy().to_string(),
        ));
    }

    let bytes = std::fs::read(wav_path)?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(DubbingMixError::WavParseError(
            "Invalid RIFF/WAVE header".to_string(),
        ));
    }

    let mut cursor = 12;
    let mut sample_rate = 48000u32;
    let mut channels = 2u16;
    let mut bit_depth = 16u16;
    let mut data_start = 0;
    let mut data_len = 0;

    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_size =
            u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let next_chunk = cursor + 8 + chunk_size;

        if chunk_id == b"fmt " && cursor + 8 + 16 <= bytes.len() {
            let fmt_cursor = cursor + 8;
            channels =
                u16::from_le_bytes(bytes[fmt_cursor + 2..fmt_cursor + 4].try_into().unwrap());
            sample_rate =
                u32::from_le_bytes(bytes[fmt_cursor + 4..fmt_cursor + 8].try_into().unwrap());
            bit_depth =
                u16::from_le_bytes(bytes[fmt_cursor + 14..fmt_cursor + 16].try_into().unwrap());
        } else if chunk_id == b"data" {
            data_start = cursor + 8;
            data_len = chunk_size.min(bytes.len().saturating_sub(data_start));
            break;
        }

        cursor = next_chunk;
    }

    if data_start == 0 || data_len == 0 {
        return Err(DubbingMixError::WavParseError(
            "Missing or empty data chunk in WAV file".to_string(),
        ));
    }

    let sample_bytes = 2usize; // 16-bit PCM
    let bytes_per_frame = (channels as usize) * sample_bytes;
    let total_frames = data_len / bytes_per_frame;
    let duration_us = if sample_rate > 0 {
        ((total_frames as u64) * 1_000_000) / (sample_rate as u64)
    } else {
        0
    };

    let pcm_data = &bytes[data_start..data_start + data_len];
    let num_samples = data_len / 2;

    let mut max_abs: i16 = 0;
    let mut sum_sq: f64 = 0.0;
    let mut clipping_samples: u64 = 0;

    for i in 0..num_samples {
        let s = i16::from_le_bytes([pcm_data[i * 2], pcm_data[i * 2 + 1]]);
        let abs_s = s.unsigned_abs();
        if abs_s > (max_abs as u16) {
            max_abs = s.abs();
        }
        if abs_s >= 32767 {
            clipping_samples += 1;
        }
        let norm = (s as f64) / 32768.0;
        sum_sq += norm * norm;
    }

    let peak_linear = (max_abs as f32) / 32768.0;
    let peak_dbfs = linear_to_db(peak_linear);

    let rms_linear = if num_samples > 0 {
        ((sum_sq / (num_samples as f64)).sqrt()) as f32
    } else {
        0.0
    };
    let rms_dbfs = linear_to_db(rms_linear);

    // Approximate integrated LUFS from K-weighted RMS (~ RMS - 0.69 dB for pink-like speech spectrum)
    let approx_lufs = if rms_linear > 0.00001 {
        Some(rms_dbfs - 0.7)
    } else {
        None
    };

    Ok(AudioAnalysisReport {
        sample_rate,
        channels,
        bit_depth,
        duration_us,
        peak_dbfs,
        rms_dbfs,
        integrated_lufs: approx_lufs,
        clipping_samples,
    })
}
