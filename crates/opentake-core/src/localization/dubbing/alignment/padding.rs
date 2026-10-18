//! Silence padding engine for Dubbing Duration Alignment.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::ai::tts::audio::validate_wav_file;
use crate::ai::tts::models::WavAudioMetadata;

use super::errors::DubbingAlignmentError;

/// Applies silence padding to a 16-bit PCM WAV file by prepending and/or appending exact zero samples.
pub fn pad_audio_with_silence(
    input_path: &Path,
    output_path: &Path,
    padding_before_us: u64,
    padding_after_us: u64,
) -> Result<WavAudioMetadata, DubbingAlignmentError> {
    if !input_path.exists() {
        return Err(DubbingAlignmentError::AudioFileNotFound(
            input_path.to_string_lossy().to_string(),
        ));
    }

    if padding_before_us == 0 && padding_after_us == 0 {
        if input_path != output_path {
            std::fs::copy(input_path, output_path)?;
        }
        let meta = validate_wav_file(output_path)
            .map_err(|e| DubbingAlignmentError::InvalidOutputWav(e.to_string()))?;
        return Ok(meta);
    }

    // Inspect input file
    let input_meta = validate_wav_file(input_path)
        .map_err(|e| DubbingAlignmentError::InvalidOutputWav(e.to_string()))?;

    let file = File::open(input_path)?;
    let mut reader = BufReader::new(file);

    // Read RIFF and chunk headers to isolate PCM payload
    let mut header = [0u8; 12];
    reader.read_exact(&mut header)?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(DubbingAlignmentError::InvalidOutputWav(
            "Input is not a valid RIFF/WAVE file".into(),
        ));
    }

    let mut audio_data = Vec::new();
    let mut chunk_header = [0u8; 8];

    while let Ok(()) = reader.read_exact(&mut chunk_header) {
        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        if chunk_id == b"data" {
            audio_data.resize(chunk_size as usize, 0u8);
            reader.read_exact(&mut audio_data)?;
            break;
        } else {
            // Skip non-data chunk
            let mut skip_buf = vec![0u8; chunk_size as usize];
            reader.read_exact(&mut skip_buf)?;
        }
    }

    let sample_rate = input_meta.sample_rate;
    let channels = input_meta.channel_count;
    let bits_per_sample = input_meta.bits_per_sample;
    let bytes_per_sample = (bits_per_sample / 8) as usize;
    let frame_size = bytes_per_sample * (channels as usize);

    // Compute sample counts for before and after padding
    let before_frames = ((padding_before_us as u128 * sample_rate as u128) / 1_000_000) as usize;
    let after_frames = ((padding_after_us as u128 * sample_rate as u128) / 1_000_000) as usize;

    let before_bytes = vec![0u8; before_frames * frame_size];
    let after_bytes = vec![0u8; after_frames * frame_size];

    let total_data_size = (before_bytes.len() + audio_data.len() + after_bytes.len()) as u32;
    let riff_size = 36 + total_data_size;
    let byte_rate = sample_rate * (frame_size as u32);

    // Write output WAV file
    let out_file = File::create(output_path)?;
    let mut writer = BufWriter::new(out_file);

    // RIFF chunk descriptor
    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    // fmt subchunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    writer.write_all(&1u16.to_le_bytes())?; // AudioFormat (1 for PCM)
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&(frame_size as u16).to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;

    // data subchunk
    writer.write_all(b"data")?;
    writer.write_all(&total_data_size.to_le_bytes())?;

    // Write payload: before silence + original audio + after silence
    if !before_bytes.is_empty() {
        writer.write_all(&before_bytes)?;
    }
    writer.write_all(&audio_data)?;
    if !after_bytes.is_empty() {
        writer.write_all(&after_bytes)?;
    }

    writer.flush()?;

    let final_meta = validate_wav_file(output_path)
        .map_err(|e| DubbingAlignmentError::InvalidOutputWav(e.to_string()))?;

    Ok(final_meta)
}
