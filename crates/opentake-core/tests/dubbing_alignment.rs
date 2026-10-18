use std::fs;
use std::path::PathBuf;

use opentake_core::localization::dubbing::alignment::{
    build_atempo_filter, build_tempo_process_args, compute_alignment_cache_key,
    evaluate_alignment_diagnostics, pad_audio_with_silence, plan_segment_alignment,
    AlignmentConfig, AlignmentSeverity, DubbingAlignmentDiagnosticCode, DubbingAlignmentMode,
    DubbingAlignmentPlan, DubbingAlignmentResult, DubbingAlignmentStatus, SilencePaddingMode,
};
use opentake_core::localization::dubbing::{
    apply_operation, DubbingAudioAsset, DubbingOperation, DubbingProject, DubbingSegment,
    DubbingSegmentStatus, DubbingTrack,
};

fn create_test_project_and_segment(
    trimmed_duration_us: u64,
    cue_start_us: u64,
    cue_end_us: u64,
) -> (DubbingProject, DubbingSegment, DubbingAudioAsset) {
    let mut project = DubbingProject::new("proj_1", "en", "es", None);
    let track_id = "track_1".to_string();
    let mut track = DubbingTrack::new(track_id.clone(), "Dubbing Track", "es");
    track.assigned_voice_id = Some("piper-en-lessac-medium".to_string());

    let asset_id = "asset_orig_1".to_string();
    let audio_asset = DubbingAudioAsset {
        asset_id: asset_id.clone(),
        file_path: PathBuf::from("/tmp/fake_asset.wav"),
        file_name: "fake_asset.wav".to_string(),
        sha256: "fake_sha256_hash_1234567890abcdef".to_string(),
        sample_rate: 22_050,
        channel_count: 1,
        duration_us: 10_000_000,
        size_bytes: 441_000,
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 1000,
    };
    project.assets.insert(asset_id.clone(), audio_asset.clone());

    let segment = DubbingSegment {
        id: "seg_align_1".to_string(),
        track_id: track_id.clone(),
        source_subtitle_cue_id: Some("cue_1".to_string()),
        source_subtitle_track_id: None,
        source_text: "Test dialogue for alignment".to_string(),
        source_text_hash: "hash_dialogue".to_string(),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        audio_asset_id: asset_id.clone(),
        source_in_us: 0,
        source_out_us: trimmed_duration_us,
        timeline_start_us: cue_start_us,
        original_cue_start_us: Some(cue_start_us),
        original_cue_end_us: Some(cue_end_us),
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: Some(asset_id),
        aligned_audio_asset_id: None,
        alignment_mode: Some(DubbingAlignmentMode::Original),
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    track.segments.push(segment.clone());
    project.tracks.push(track);

    (project, segment, audio_asset)
}

#[test]
fn test_atempo_filter_construction() {
    // Normal 1.15x
    let f1 = build_atempo_filter(1.15).unwrap();
    assert_eq!(f1, "atempo=1.1500");

    // Normal 0.90x
    let f2 = build_atempo_filter(0.90).unwrap();
    assert_eq!(f2, "atempo=0.9000");

    // Extreme 2.5x -> chained 2.0 and 1.25
    let f3 = build_atempo_filter(2.5).unwrap();
    assert!(f3.contains("atempo=2.0,atempo=1.2500"));

    // Extreme 0.25x -> chained 0.5 and 0.5
    let f4 = build_atempo_filter(0.25).unwrap();
    assert!(f4.contains("atempo=0.5,atempo=0.5000"));

    // Negative / zero rejection
    assert!(build_atempo_filter(0.0).is_err());
    assert!(build_atempo_filter(-1.0).is_err());
}

#[test]
fn test_ffmpeg_argument_builder_safety() {
    let input = PathBuf::from("/path/to/input.wav");
    let output = PathBuf::from("/path/to/output.wav");
    let args =
        build_tempo_process_args(&input, &output, 500_000, 3_500_000, 1.12, 22050, 1).unwrap();

    // Check non-shell structured flags
    assert_eq!(args[0], "-y");
    assert!(args.contains(&"-ss".to_string()));
    assert!(args.contains(&"0.500000".to_string()));
    assert!(args.contains(&"-t".to_string()));
    assert!(args.contains(&"3.000000".to_string()));
    assert!(args.contains(&"pcm_s16le".to_string()));
    assert!(args.contains(&"atempo=1.1200".to_string()));
}

#[test]
fn test_planner_speech_matches_cue_tolerance() {
    let config = AlignmentConfig::default();
    // 3.0s speech in 3.01s cue (within 25ms tolerance)
    let (_proj, segment, asset) = create_test_project_and_segment(3_000_000, 1_000_000, 4_010_000);

    let plan = plan_segment_alignment(&segment, &asset, &config, None, None, None, None).unwrap();

    assert_eq!(plan.proposed_tempo, 1.0);
    assert_eq!(plan.padding_mode, SilencePaddingMode::NoPadding);
    assert_eq!(plan.padding_before_us, 0);
    assert_eq!(plan.padding_after_us, 0);
    assert!(!plan.requires_manual_review);
    assert!(plan.selected_strategy.contains("tolerance"));
}

#[test]
fn test_planner_short_speech_silence_padding() {
    let config = AlignmentConfig::default();
    // 2.0s speech in 3.5s cue (shortfall of 1.5s)
    let (_proj, segment, asset) = create_test_project_and_segment(2_000_000, 1_000_000, 4_500_000);

    // Default PadEnd
    let plan = plan_segment_alignment(&segment, &asset, &config, None, None, None, None).unwrap();
    assert_eq!(plan.proposed_tempo, 1.0);
    assert_eq!(plan.padding_mode, SilencePaddingMode::PadEnd);
    assert_eq!(plan.padding_before_us, 0);
    assert_eq!(plan.padding_after_us, 1_500_000);
    assert!(!plan.requires_manual_review);

    // Centered padding override
    let plan_centered = plan_segment_alignment(
        &segment,
        &asset,
        &config,
        None,
        Some(SilencePaddingMode::Centered),
        None,
        None,
    )
    .unwrap();
    assert_eq!(plan_centered.padding_mode, SilencePaddingMode::Centered);
    assert_eq!(plan_centered.padding_before_us, 750_000);
    assert_eq!(plan_centered.padding_after_us, 750_000);
}

#[test]
fn test_planner_moderate_overrun_autofit() {
    let config = AlignmentConfig::default();
    // 3.3s speech in 3.0s cue -> requires 3.3 / 3.0 = 1.10x tempo (within 1.15x auto limit)
    let (_proj, segment, asset) = create_test_project_and_segment(3_300_000, 1_000_000, 4_000_000);

    let plan = plan_segment_alignment(&segment, &asset, &config, None, None, None, None).unwrap();

    assert!((plan.proposed_tempo - 1.10).abs() < 0.01);
    assert_eq!(plan.padding_mode, SilencePaddingMode::NoPadding);
    assert!(!plan.requires_manual_review);
    assert!(plan.selected_strategy.contains("1.10x"));
}

#[test]
fn test_planner_large_overrun_flags_needs_review() {
    let config = AlignmentConfig::default();
    // 4.5s speech in 3.0s cue -> requires 4.5 / 3.0 = 1.50x tempo (> 1.15x auto limit)
    let (_proj, segment, asset) = create_test_project_and_segment(4_500_000, 1_000_000, 4_000_000);

    let plan = plan_segment_alignment(&segment, &asset, &config, None, None, None, None).unwrap();

    assert!((plan.proposed_tempo - 1.50).abs() < 0.01);
    assert!(plan.requires_manual_review);
    assert!(plan
        .warnings
        .iter()
        .any(|w| w.contains("exceeds conservative AutoFit limit")));
}

#[test]
fn test_planner_extreme_overrun_rejection() {
    let config = AlignmentConfig::default();
    // 7.0s speech in 2.0s cue -> requires 3.5x tempo (> 2.00x max supported)
    let (_proj, segment, asset) = create_test_project_and_segment(7_000_000, 1_000_000, 3_000_000);

    let plan = plan_segment_alignment(&segment, &asset, &config, None, None, None, None).unwrap();

    assert_eq!(plan.proposed_tempo, 1.0);
    assert!(plan.requires_manual_review);
    assert!(plan.selected_strategy.contains("infeasible"));
}

#[test]
fn test_planner_manual_start_offset() {
    let config = AlignmentConfig::default();
    // 3.0s speech, 4.0s cue window, +500ms manual start offset -> effective window is 3.5s
    let (_proj, segment, asset) = create_test_project_and_segment(3_000_000, 1_000_000, 5_000_000);

    let plan =
        plan_segment_alignment(&segment, &asset, &config, None, None, Some(500_000), None).unwrap();

    assert_eq!(plan.requested_start_offset_us, 500_000);
    assert_eq!(plan.effective_target_duration_us, 3_500_000);
    // 3.0s speech in 3.5s effective window -> shortfall of 500ms
    assert_eq!(plan.padding_after_us, 500_000);
}

#[test]
fn test_cache_key_determinism() {
    let k1 = compute_alignment_cache_key("sha123", 0, 3000000, 1.12, 0, 0, 1);
    let k2 = compute_alignment_cache_key("sha123", 0, 3000000, 1.12, 0, 0, 1);
    let k3 = compute_alignment_cache_key("sha123", 0, 3000000, 1.15, 0, 0, 1); // different tempo

    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn test_silence_padding_pcm_math() {
    let temp_dir = tempfile::tempdir().unwrap();
    let src_wav = temp_dir.path().join("source.wav");
    let out_wav = temp_dir.path().join("padded.wav");

    // Write a tiny valid 16-bit PCM mono WAV file (44 bytes header + 22,050 samples = 1.0s at 22,050 Hz)
    let sample_rate: u32 = 22_050;
    let samples: Vec<i16> = (0..22_050).map(|i| ((i % 100) * 100) as i16).collect();
    let sample_bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();

    let mut wav_file = Vec::new();
    let data_size = sample_bytes.len() as u32;
    let riff_size = 36 + data_size;
    wav_file.extend_from_slice(b"RIFF");
    wav_file.extend_from_slice(&riff_size.to_le_bytes());
    wav_file.extend_from_slice(b"WAVE");
    wav_file.extend_from_slice(b"fmt ");
    wav_file.extend_from_slice(&16u32.to_le_bytes());
    wav_file.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav_file.extend_from_slice(&1u16.to_le_bytes()); // Mono
    wav_file.extend_from_slice(&sample_rate.to_le_bytes());
    wav_file.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // Byte rate
    wav_file.extend_from_slice(&2u16.to_le_bytes()); // Block align
    wav_file.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample
    wav_file.extend_from_slice(b"data");
    wav_file.extend_from_slice(&data_size.to_le_bytes());
    wav_file.extend_from_slice(&sample_bytes);
    fs::write(&src_wav, &wav_file).unwrap();

    // Pad with 500ms before and 500ms after
    let meta = pad_audio_with_silence(&src_wav, &out_wav, 500_000, 500_000).unwrap();

    // Original: 22,050 samples (1.0s)
    // Padded: 22,050 + 11,025 + 11,025 = 44,100 samples (2.0s)
    assert_eq!(meta.sample_count, 44_100);
    assert_eq!(meta.duration_us, 2_000_000);
    assert_eq!(meta.sample_rate, 22_050);
}

#[test]
fn test_operations_apply_alignment_and_restore_original() {
    let (mut project, segment, asset) =
        create_test_project_and_segment(3_000_000, 1_000_000, 4_000_000);

    let plan = DubbingAlignmentPlan {
        segment_id: segment.id.clone(),
        original_asset_id: asset.asset_id.clone(),
        original_asset_hash: asset.sha256.clone(),
        source_trimmed_duration_us: 3_000_000,
        target_start_us: 1_000_000,
        target_end_us: 4_000_000,
        target_duration_us: 3_000_000,
        requested_start_offset_us: 0,
        effective_target_duration_us: 3_000_000,
        proposed_tempo: 1.0,
        padding_mode: SilencePaddingMode::NoPadding,
        padding_before_us: 0,
        padding_after_us: 0,
        selected_strategy: "AutoFit".to_string(),
        requires_manual_review: false,
        warnings: Vec::new(),
        algorithm_version: 1,
    };

    let aligned_asset_id = "aligned_test_123".to_string();
    let aligned_asset = DubbingAudioAsset {
        asset_id: aligned_asset_id.clone(),
        file_path: PathBuf::from("/tmp/aligned.wav"),
        file_name: "aligned.wav".to_string(),
        sha256: "aligned_sha_123".to_string(),
        sample_rate: 22_050,
        channel_count: 1,
        duration_us: 3_000_000,
        size_bytes: 132_300,
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 2000,
    };

    let result = DubbingAlignmentResult {
        alignment_id: "exec_1".to_string(),
        segment_id: segment.id.clone(),
        original_asset_id: asset.asset_id.clone(),
        aligned_asset_id: Some(aligned_asset_id.clone()),
        actual_output_duration_us: 3_000_000,
        effective_placement_start_us: 1_000_000,
        effective_placement_end_us: 4_000_000,
        processing_method: "atempo=1.0".to_string(),
        measured_duration_error_us: 0,
        tempo_factor_applied: 1.0,
        padding_before_applied_us: 0,
        padding_after_applied_us: 0,
        diagnostics: Vec::new(),
        status: DubbingAlignmentStatus::Aligned,
        created_at_epoch_ms: 2000,
    };

    // 1. Apply alignment
    let op_apply = DubbingOperation::ApplyAlignment {
        segment_id: segment.id.clone(),
        plan,
        result,
        aligned_asset,
    };
    apply_operation(&mut project, &op_apply).unwrap();

    let seg = project.find_segment(&segment.id).unwrap().1;
    assert_eq!(seg.audio_asset_id, aligned_asset_id);
    assert_eq!(
        seg.aligned_audio_asset_id.as_deref(),
        Some(aligned_asset_id.as_str())
    );
    assert_eq!(
        seg.original_audio_asset_id.as_deref(),
        Some(asset.asset_id.as_str())
    );
    assert!(seg.is_aligned());
    assert_eq!(seg.active_audio_asset_id(), aligned_asset_id);

    // 2. Set mode to Original (temporary A/B preview switch)
    let op_mode_orig = DubbingOperation::SetAlignmentMode {
        segment_id: segment.id.clone(),
        mode: DubbingAlignmentMode::Original,
    };
    apply_operation(&mut project, &op_mode_orig).unwrap();

    let seg_orig = project.find_segment(&segment.id).unwrap().1;
    assert_eq!(seg_orig.active_audio_asset_id(), asset.asset_id);
    assert!(!seg_orig.is_aligned());

    // 3. Restore Original Audio completely
    let op_restore = DubbingOperation::RestoreOriginalAudio {
        segment_id: segment.id.clone(),
    };
    apply_operation(&mut project, &op_restore).unwrap();

    let seg_restored = project.find_segment(&segment.id).unwrap().1;
    assert_eq!(seg_restored.audio_asset_id, asset.asset_id);
    assert_eq!(seg_restored.source_in_us, 0);
    assert_eq!(seg_restored.source_out_us, asset.duration_us);
}

#[test]
fn test_evaluate_alignment_diagnostics() {
    let config = AlignmentConfig::default();
    // 5.0s speech in 3.0s cue (overrun of 2.0s)
    let (project, _segment, _asset) =
        create_test_project_and_segment(5_000_000, 1_000_000, 4_000_000);

    let findings = evaluate_alignment_diagnostics(&project, &config);

    assert!(findings
        .iter()
        .any(|f| f.code == DubbingAlignmentDiagnosticCode::SpeechLongerThanCue));
    let overrun_finding = findings
        .iter()
        .find(|f| f.code == DubbingAlignmentDiagnosticCode::SpeechLongerThanCue)
        .unwrap();
    assert_eq!(overrun_finding.severity, AlignmentSeverity::Warning);
    assert_eq!(overrun_finding.duration_diff_us, Some(2_000_000));
}
