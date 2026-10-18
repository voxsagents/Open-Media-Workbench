use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use opentake_core::localization::dubbing::alignment::{
    DubbingAlignmentMode, DubbingAlignmentResult, DubbingAlignmentStatus,
};
use opentake_core::localization::dubbing::mixing::{
    analyze_pcm_wav_file, plan_dubbing_mix, render_dubbing_mixdown, DubbingMixConfiguration,
    DubbingMixTrackSettings,
};
use opentake_core::localization::dubbing::operations::{apply_operation, DubbingOperation};
use opentake_core::localization::dubbing::persistence::{
    load_dubbing_project_from_file, save_dubbing_project_to_file,
};
use opentake_core::localization::dubbing::{
    DubbingAudioAsset, DubbingProject, DubbingSegment, DubbingSegmentStatus, DubbingTrack,
};

fn generate_test_wav(
    output_path: &Path,
    freq_hz: u32,
    duration_secs: f64,
    sample_rate: u32,
    channels: u16,
) -> bool {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency={freq_hz}:duration={duration_secs}"),
            "-c:a",
            "pcm_s16le",
            "-ar",
            &sample_rate.to_string(),
            "-ac",
            &channels.to_string(),
            output_path.to_str().unwrap(),
        ])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

fn compute_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let data = fs::read(path).expect("Failed to read file for hashing");
    let mut hasher = Sha256::new();
    hasher.update(&data);
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn test_real_ffmpeg_mixing_e2e_cases_a_through_f() {
    let temp_root = tempdir().unwrap();
    let durable_dir = temp_root.path().join("durable_assets");
    let media_dir = temp_root.path().join("media");
    fs::create_dir_all(&durable_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    // 1. Generate real original soundtrack (440 Hz sine wave, 5 seconds, 48kHz stereo)
    let orig_soundtrack_path = media_dir.join("original_video_audio.wav");
    let orig_created = generate_test_wav(&orig_soundtrack_path, 440, 5.0, 48000, 2);
    if !orig_created {
        eprintln!("System FFmpeg not available or failed to generate test fixture. Skipping test.");
        return;
    }
    let orig_sha256_before = compute_sha256(&orig_soundtrack_path);

    // 2. Generate real AI speech segment 1 (880 Hz tone, 1.5 seconds, 22050Hz mono, mimicking Piper)
    let speech_1_path = durable_dir.join("piper_voice_segment_1.wav");
    assert!(generate_test_wav(&speech_1_path, 880, 1.5, 22050, 1));
    let speech_1_analysis = analyze_pcm_wav_file(&speech_1_path).unwrap();

    // 3. Generate real AI speech segment 2 (1200 Hz tone, 1.2 seconds, 22050Hz mono)
    let speech_2_path = durable_dir.join("piper_voice_segment_2.wav");
    assert!(generate_test_wav(&speech_2_path, 1200, 1.2, 22050, 1));
    let speech_2_analysis = analyze_pcm_wav_file(&speech_2_path).unwrap();

    // 4. Generate real aligned asset for segment 3 (aligned tempo 1000 Hz tone, 2.0 seconds)
    let speech_aligned_path = durable_dir.join("piper_voice_aligned.wav");
    assert!(generate_test_wav(&speech_aligned_path, 1000, 2.0, 22050, 1));
    let speech_aligned_analysis = analyze_pcm_wav_file(&speech_aligned_path).unwrap();

    // Setup DubbingProject
    let mut project = DubbingProject::new(
        "proj_e2e_mix",
        "en",
        "es",
        Some(orig_soundtrack_path.to_str().unwrap().to_string()),
    );
    project.timeline_duration_us = 5_000_000;

    // Register audio assets
    let asset_1 = DubbingAudioAsset {
        asset_id: "asset_voice_1".to_string(),
        file_path: speech_1_path.clone(),
        file_name: "piper_voice_segment_1.wav".to_string(),
        sha256: compute_sha256(&speech_1_path),
        sample_rate: speech_1_analysis.sample_rate,
        channel_count: speech_1_analysis.channels,
        duration_us: speech_1_analysis.duration_us,
        size_bytes: fs::metadata(&speech_1_path).map(|m| m.len()).unwrap_or(0),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 1000,
    };
    project.assets.insert(asset_1.asset_id.clone(), asset_1);

    let asset_2 = DubbingAudioAsset {
        asset_id: "asset_voice_2".to_string(),
        file_path: speech_2_path.clone(),
        file_name: "piper_voice_segment_2.wav".to_string(),
        sha256: compute_sha256(&speech_2_path),
        sample_rate: speech_2_analysis.sample_rate,
        channel_count: speech_2_analysis.channels,
        duration_us: speech_2_analysis.duration_us,
        size_bytes: fs::metadata(&speech_2_path).map(|m| m.len()).unwrap_or(0),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 2000,
    };
    project.assets.insert(asset_2.asset_id.clone(), asset_2);

    let asset_aligned = DubbingAudioAsset {
        asset_id: "asset_voice_aligned".to_string(),
        file_path: speech_aligned_path.clone(),
        file_name: "piper_voice_aligned.wav".to_string(),
        sha256: compute_sha256(&speech_aligned_path),
        sample_rate: speech_aligned_analysis.sample_rate,
        channel_count: speech_aligned_analysis.channels,
        duration_us: speech_aligned_analysis.duration_us,
        size_bytes: fs::metadata(&speech_aligned_path)
            .map(|m| m.len())
            .unwrap_or(0),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 3000,
    };
    project
        .assets
        .insert(asset_aligned.asset_id.clone(), asset_aligned);

    // Track 1: Lead Dialogue
    let mut track_1 = DubbingTrack::new("track_lead", "Lead Dialogue", "es");
    track_1.assigned_voice_id = Some("piper-en-lessac-medium".to_string());

    // Segment 1: at 1.0s to 2.5s
    let seg_1 = DubbingSegment {
        id: "seg_1".to_string(),
        track_id: "track_lead".to_string(),
        source_subtitle_cue_id: Some("cue_1".to_string()),
        source_subtitle_track_id: None,
        source_text: "Hello world".to_string(),
        source_text_hash: "hash_1".to_string(),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        audio_asset_id: "asset_voice_1".to_string(),
        source_in_us: 0,
        source_out_us: 1_500_000,
        timeline_start_us: 1_000_000,
        original_cue_start_us: Some(1_000_000),
        original_cue_end_us: Some(2_500_000),
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: Some("asset_voice_1".to_string()),
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    track_1.segments.push(seg_1);
    project.tracks.push(track_1);

    // Track 2: Secondary Dialogue
    let mut track_2 = DubbingTrack::new("track_sec", "Secondary Dialogue", "es");
    track_2.assigned_voice_id = Some("piper-en-lessac-medium".to_string());

    // Segment 2: at 2.3s to 3.5s (overlapping seg 1 by 200ms)
    let seg_2 = DubbingSegment {
        id: "seg_2".to_string(),
        track_id: "track_sec".to_string(),
        source_subtitle_cue_id: Some("cue_2".to_string()),
        source_subtitle_track_id: None,
        source_text: "Second voice overlapping".to_string(),
        source_text_hash: "hash_2".to_string(),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        audio_asset_id: "asset_voice_2".to_string(),
        source_in_us: 0,
        source_out_us: 1_200_000,
        timeline_start_us: 2_300_000,
        original_cue_start_us: Some(2_300_000),
        original_cue_end_us: Some(3_500_000),
        gain: 1.1,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: Some("asset_voice_2".to_string()),
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    track_2.segments.push(seg_2);
    project.tracks.push(track_2);

    let cancel = CancellationToken::new();

    // ==========================================
    // CASE A & B & C: Real Mixdown with Ducking
    // ==========================================
    let mut config_ducking = DubbingMixConfiguration::new("proj_e2e_mix");
    config_ducking.original_audio_enabled = true;
    config_ducking.original_audio_gain_db = -3.0;
    config_ducking.dubbing_master_gain_db = 0.0;
    config_ducking.ducking_enabled = true;
    config_ducking.ducking_attenuation_db = -12.0;
    config_ducking.ducking_attack_ms = 40;
    config_ducking.ducking_release_ms = 150;
    config_ducking.ducking_hold_ms = 100;

    let plan_ducking = plan_dubbing_mix(&project, &config_ducking, Some(&orig_soundtrack_path))
        .await
        .expect("Planning mix with ducking must succeed");

    assert_eq!(plan_ducking.segments.len(), 2);
    assert_eq!(
        plan_ducking.ducking_intervals.len(),
        1,
        "Overlapping speech at 1.0s-2.5s and 2.3s-3.5s must merge into 1 envelope"
    );
    assert!(plan_ducking.has_original_audio);

    let result_ducking = render_dubbing_mixdown(&plan_ducking, &durable_dir, cancel.clone())
        .await
        .expect("Mixdown rendering with ducking must succeed");

    assert!(
        result_ducking.file_path.exists(),
        "Rendered WAV must exist on disk"
    );
    assert!(
        result_ducking.actual_duration_us >= 4_900_000,
        "Output WAV duration must match timeline duration ~5s"
    );
    assert_eq!(
        result_ducking.sample_rate, 48000,
        "Sample rate must be 48kHz"
    );
    assert_eq!(result_ducking.channels, 2, "Channel count must be stereo");
    assert_eq!(
        result_ducking.clipping_samples, 0,
        "Alimiter must prevent clipping"
    );
    assert!(
        result_ducking.peak_dbfs <= -0.9,
        "Peak level must be below ceiling (-1.0 dBFS)"
    );

    // Confirm original source media was NOT modified
    let orig_sha256_after = compute_sha256(&orig_soundtrack_path);
    assert_eq!(
        orig_sha256_before, orig_sha256_after,
        "Original source media SHA-256 must be byte-for-byte unchanged"
    );

    // ==========================================
    // CASE D: Track Mute & Solo Semantics
    // ==========================================
    let mut config_solo = config_ducking.clone();
    config_solo.track_settings.insert(
        "track_lead".to_string(),
        DubbingMixTrackSettings {
            track_id: "track_lead".to_string(),
            is_enabled: true,
            is_muted: false,
            is_solo: true,
            gain_db: 0.0,
            pan: 0.0,
        },
    );

    let plan_solo = plan_dubbing_mix(&project, &config_solo, Some(&orig_soundtrack_path))
        .await
        .expect("Planning solo mix must succeed");

    assert_eq!(
        plan_solo.segments.len(),
        1,
        "Only soloed track_lead should participate"
    );
    assert_eq!(plan_solo.segments[0].segment_id, "seg_1");

    let result_solo = render_dubbing_mixdown(&plan_solo, &durable_dir, cancel.clone())
        .await
        .expect("Solo mixdown rendering must succeed");
    assert!(result_solo.file_path.exists());

    // ==========================================
    // CASE E: Video Without Original Audio
    // ==========================================
    let mut config_no_orig = config_ducking.clone();
    config_no_orig.original_audio_enabled = false;

    let plan_no_orig = plan_dubbing_mix(
        &project,
        &config_no_orig,
        None, // No media audio
    )
    .await
    .expect("Planning speech-only mix must succeed");

    assert!(!plan_no_orig.has_original_audio);
    let result_no_orig = render_dubbing_mixdown(&plan_no_orig, &durable_dir, cancel.clone())
        .await
        .expect("Speech-only mixdown must succeed");

    assert!(result_no_orig.file_path.exists());
    assert_eq!(result_no_orig.channels, 2);
    assert_eq!(result_no_orig.sample_rate, 48000);

    // ==========================================
    // CASE F: Aligned Asset Placement
    // ==========================================
    let mut project_aligned = project.clone();
    project_aligned.tracks[0].segments[0].aligned_audio_asset_id =
        Some("asset_voice_aligned".to_string());
    project_aligned.tracks[0].segments[0].alignment_mode = Some(DubbingAlignmentMode::AutoFit);
    project_aligned.tracks[0].segments[0].alignment_result = Some(DubbingAlignmentResult {
        alignment_id: "align_res_1".to_string(),
        segment_id: "seg_1".to_string(),
        original_asset_id: "asset_voice_1".to_string(),
        aligned_asset_id: Some("asset_voice_aligned".to_string()),
        actual_output_duration_us: 2_000_000,
        effective_placement_start_us: 1_000_000,
        effective_placement_end_us: 3_000_000,
        processing_method: "atempo=1.00".to_string(),
        measured_duration_error_us: 0,
        tempo_factor_applied: 1.0,
        padding_before_applied_us: 0,
        padding_after_applied_us: 0,
        diagnostics: vec![],
        status: DubbingAlignmentStatus::Aligned,
        created_at_epoch_ms: 3000,
    });

    let plan_aligned = plan_dubbing_mix(
        &project_aligned,
        &config_ducking,
        Some(&orig_soundtrack_path),
    )
    .await
    .expect("Planning aligned mix must succeed");

    assert!(plan_aligned.segments[0].is_aligned);
    assert_eq!(plan_aligned.segments[0].asset_id, "asset_voice_aligned");

    let result_aligned = render_dubbing_mixdown(&plan_aligned, &durable_dir, cancel.clone())
        .await
        .expect("Aligned mixdown must succeed");

    assert!(result_aligned.file_path.exists());

    // ==========================================
    // Persistence & Reopen Verification
    // ==========================================
    let op_config = DubbingOperation::UpdateMixConfiguration {
        configuration: Box::new(config_ducking.clone()),
    };
    apply_operation(&mut project, &op_config).expect("Applying mix config must succeed");

    let op_res = DubbingOperation::ApplyMixResult {
        result: Box::new(result_ducking.clone()),
    };
    apply_operation(&mut project, &op_res).expect("Applying mix result must succeed");

    let proj_file = temp_root.path().join("test_project.omwdub");
    save_dubbing_project_to_file(&project, &proj_file).expect("Saving project must succeed");

    let reloaded =
        load_dubbing_project_from_file(&proj_file).expect("Reloading project must succeed");
    assert!(reloaded.mix_configuration.is_some());
    assert_eq!(
        reloaded
            .mix_configuration
            .as_ref()
            .unwrap()
            .ducking_attenuation_db,
        -12.0
    );
    assert!(reloaded.mix_result.is_some());
    assert_eq!(
        reloaded.mix_result.as_ref().unwrap().mix_asset_id,
        result_ducking.mix_asset_id
    );
    assert!(reloaded.mix_result.as_ref().unwrap().file_path.exists());
}
