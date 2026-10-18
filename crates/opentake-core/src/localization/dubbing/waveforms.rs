//! Real audio waveform peak and RMS extraction from PCM WAV files.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::errors::DubbingError;
use super::models::{DubbingTimelineWaveform, DubbingWaveformBin};

/// Number of waveform bins generated per second of audio (50 bins/sec = 20ms resolution).
pub const DUBBING_WAVEFORM_BINS_PER_SECOND: u32 = 50;

/// Reads raw 16-bit PCM WAV audio and computes normalized waveform envelope bins.
pub fn generate_waveform_from_wav(
    path: &Path,
    segment_id: impl Into<String>,
    asset_id: impl Into<String>,
) -> Result<DubbingTimelineWaveform, DubbingError> {
    if !path.exists() {
        return Err(DubbingError::AudioValidationFailed(format!(
            "WAV file not found: {}",
            path.display()
        )));
    }

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut header = [0u8; 12];
    reader.read_exact(&mut header)?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(DubbingError::AudioValidationFailed(
            "Invalid WAV header signature".into(),
        ));
    }

    let mut sample_rate: Option<u32> = None;
    let mut channel_count: Option<u16> = None;
    let mut bits_per_sample: Option<u16> = None;
    let mut data_len: Option<u32> = None;
    let mut data_offset: Option<u64> = None;

    while let Ok(()) = reader.read_exact(&mut header[0..8]) {
        let chunk_id = &header[0..4];
        let chunk_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

        if chunk_id == b"fmt " {
            let mut fmt_data = vec![0u8; chunk_size as usize];
            reader.read_exact(&mut fmt_data)?;

            channel_count = Some(u16::from_le_bytes([fmt_data[2], fmt_data[3]]));
            sample_rate = Some(u32::from_le_bytes([
                fmt_data[4],
                fmt_data[5],
                fmt_data[6],
                fmt_data[7],
            ]));
            bits_per_sample = Some(u16::from_le_bytes([fmt_data[14], fmt_data[15]]));
        } else if chunk_id == b"data" {
            data_len = Some(chunk_size);
            data_offset = Some(reader.stream_position()?);
            reader.seek(SeekFrom::Current(chunk_size as i64))?;
        } else {
            reader.seek(SeekFrom::Current(chunk_size as i64))?;
        }
    }

    let sample_rate = sample_rate.unwrap_or(22050);
    let channel_count = channel_count.unwrap_or(1);
    let bits_per_sample = bits_per_sample.unwrap_or(16);
    let data_len = data_len.unwrap_or(0);
    let data_offset = data_offset.unwrap_or(44);

    if bits_per_sample != 16 {
        return Err(DubbingError::AudioValidationFailed(format!(
            "Unsupported bits per sample: {bits_per_sample} (expected 16-bit PCM)"
        )));
    }

    let bytes_per_sample = 2usize;
    let frame_size = (channel_count as usize) * bytes_per_sample;
    let total_frames = (data_len as usize) / frame_size;
    let duration_us = (total_frames as f64 / sample_rate as f64 * 1_000_000.0) as u64;

    // Open file to stream audio samples
    let mut data_file = File::open(path)?;
    data_file.seek(SeekFrom::Start(data_offset))?;
    let mut data_reader = BufReader::new(data_file);

    let samples_per_bin = ((sample_rate as f64) / (DUBBING_WAVEFORM_BINS_PER_SECOND as f64))
        .max(1.0)
        .round() as usize;

    let mut bins = Vec::new();
    let mut cur_min = 0.0f32;
    let mut cur_max = 0.0f32;
    let mut cur_sum_sq = 0.0f64;
    let mut cur_count = 0usize;

    let mut sample_bytes = vec![0u8; frame_size];

    while let Ok(()) = data_reader.read_exact(&mut sample_bytes) {
        // Read first channel sample (mono or left)
        let sample_i16 = i16::from_le_bytes([sample_bytes[0], sample_bytes[1]]);
        let sample_f32 = (sample_i16 as f32) / 32768.0;

        cur_min = cur_min.min(sample_f32);
        cur_max = cur_max.max(sample_f32);
        cur_sum_sq += (sample_f32 as f64) * (sample_f32 as f64);
        cur_count += 1;

        if cur_count >= samples_per_bin {
            let rms = (cur_sum_sq / (cur_count as f64)).sqrt() as f32;
            bins.push(DubbingWaveformBin {
                min: cur_min,
                max: cur_max,
                rms: rms.clamp(0.0, 1.0),
            });
            cur_min = 0.0;
            cur_max = 0.0;
            cur_sum_sq = 0.0;
            cur_count = 0;
        }
    }

    if cur_count > 0 {
        let rms = (cur_sum_sq / (cur_count as f64)).sqrt() as f32;
        bins.push(DubbingWaveformBin {
            min: cur_min,
            max: cur_max,
            rms: rms.clamp(0.0, 1.0),
        });
    }

    Ok(DubbingTimelineWaveform {
        segment_id: segment_id.into(),
        asset_id: asset_id.into(),
        sample_rate,
        duration_us,
        bins,
    })
}
