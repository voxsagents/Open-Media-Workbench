use std::fs;
use std::path::PathBuf;

use opentake_core::localization::dubbing::alignment::{
    DubbingAlignmentMode, DubbingAlignmentResult, DubbingAlignmentStatus,
};
use opentake_core::localization::dubbing::mixing::{
    apply_preset_to_configuration, build_complex_filter_graph, build_ducking_volume_expression,
    compute_ducking_intervals, compute_mix_plan_cache_key, db_to_linear, linear_to_db,
    plan_dubbing_mix, DubbingMixConfiguration, DubbingMixPlan, DubbingMixResult, DubbingMixSegment,
    DubbingMixTrackSettings, DuckingInterval, MixPreset,
};
use opentake_core::localization::dubbing::{
    apply_operation, DubbingAudioAsset, DubbingOperation, DubbingProject, DubbingSegment,
    DubbingSegmentStatus, DubbingTrack,
};

fn create_test_project(temp_dir: &std::path::Path) -> DubbingProject {
    let mut project = DubbingProject::new("proj_mix_1", "en", "es", None);

    // Create real small dummy files for asset validation
    let file_1 = temp_dir.join("test_asset_1.wav");
    fs::write(&file_1, b"RIFFfake_asset_1_data").unwrap();
    let file_2 = temp_dir.join("test_asset_2.wav");
    fs::write(&file_2, b"RIFFfake_asset_2_data").unwrap();

    // Track 1
    let track_id_1 = "track_1".to_string();
    let mut track_1 = DubbingTrack::new(track_id_1.clone(), "Lead Voice", "es");
    track_1.assigned_voice_id = Some("piper-en-lessac-medium".to_string());

    // Track 2
    let track_id_2 = "track_2".to_string();
    let mut track_2 = DubbingTrack::new(track_id_2.clone(), "Background Voice", "es");
    track_2.assigned_voice_id = Some("piper-en-lessac-medium".to_string());

    // Assets
    let asset_1 = DubbingAudioAsset {
        asset_id: "asset_1".to_string(),
        file_path: file_1,
        file_name: "test_asset_1.wav".to_string(),
        sha256: "hash_asset_1_0123456789abcdef".to_string(),
        sample_rate: 22_050,
        channel_count: 1,
        duration_us: 2_000_000,
        size_bytes: 88_200,
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 1000,
    };
    project.assets.insert(asset_1.asset_id.clone(), asset_1);

    let asset_2 = DubbingAudioAsset {
        asset_id: "asset_2".to_string(),
        file_path: file_2,
        file_name: "test_asset_2.wav".to_string(),
        sha256: "hash_asset_2_0123456789abcdef".to_string(),
        sample_rate: 22_050,
        channel_count: 1,
        duration_us: 1_500_000,
        size_bytes: 66_150,
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 2000,
    };
    project.assets.insert(asset_2.asset_id.clone(), asset_2);

    // Segment 1 on track 1 (0.5s to 2.5s)
    let seg_1 = DubbingSegment {
        id: "seg_1".to_string(),
        track_id: track_id_1.clone(),
        source_subtitle_cue_id: Some("cue_1".to_string()),
        source_subtitle_track_id: None,
        source_text: "First speech phrase".to_string(),
        source_text_hash: "hash_1".to_string(),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        audio_asset_id: "asset_1".to_string(),
        source_in_us: 0,
        source_out_us: 2_000_000,
        timeline_start_us: 500_000,
        original_cue_start_us: Some(500_000),
        original_cue_end_us: Some(2_500_000),
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: Some("asset_1".to_string()),
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    track_1.segments.push(seg_1);
    project.tracks.push(track_1);

    // Segment 2 on track 2 (2.2s to 3.7s - overlaps with seg 1 by 300ms)
    let seg_2 = DubbingSegment {
        id: "seg_2".to_string(),
        track_id: track_id_2.clone(),
        source_subtitle_cue_id: Some("cue_2".to_string()),
        source_subtitle_track_id: None,
        source_text: "Second speech phrase".to_string(),
        source_text_hash: "hash_2".to_string(),
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        audio_asset_id: "asset_2".to_string(),
        source_in_us: 0,
        source_out_us: 1_500_000,
        timeline_start_us: 2_200_000,
        original_cue_start_us: Some(2_200_000),
        original_cue_end_us: Some(3_700_000),
        gain: 1.2,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: Some("asset_2".to_string()),
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    track_2.segments.push(seg_2);
    project.tracks.push(track_2);

    project
}

#[test]
fn test_gain_db_conversions() {
    // 0 dB = unity gain (1.0)
    assert!((db_to_linear(0.0) - 1.0).abs() < 1e-4);
    assert!((linear_to_db(1.0) - 0.0).abs() < 1e-4);

    // -6 dB approx 0.501187
    let lin_minus_6 = db_to_linear(-6.0);
    assert!((lin_minus_6 - 0.501187).abs() < 1e-4);
    assert!((linear_to_db(lin_minus_6) - (-6.0)).abs() < 1e-4);

    // +6 dB approx 1.99526
    let lin_plus_6 = db_to_linear(6.0);
    assert!((lin_plus_6 - 1.99526).abs() < 1e-4);
    assert!((linear_to_db(lin_plus_6) - 6.0).abs() < 1e-4);

    // Silent / negative infinity
    assert_eq!(db_to_linear(-100.0), 0.0);
    assert_eq!(linear_to_db(0.0), -100.0);
}

#[test]
fn test_mix_presets() {
    let mut config = DubbingMixConfiguration::new("proj_1");

    // Balanced preset
    apply_preset_to_configuration(&mut config, MixPreset::Balanced);
    assert_eq!(config.preset, MixPreset::Balanced);
    assert_eq!(config.original_audio_gain_db, -6.0);
    assert_eq!(config.dubbing_master_gain_db, 0.0);
    assert!(config.ducking_enabled);
    assert_eq!(config.ducking_attenuation_db, -12.0);

    // Voice Focus preset
    apply_preset_to_configuration(&mut config, MixPreset::VoiceFocus);
    assert_eq!(config.preset, MixPreset::VoiceFocus);
    assert_eq!(config.original_audio_gain_db, -12.0);
    assert_eq!(config.dubbing_master_gain_db, 1.5);
    assert_eq!(config.ducking_attenuation_db, -18.0);

    // Preserve Original preset
    apply_preset_to_configuration(&mut config, MixPreset::PreserveOriginal);
    assert_eq!(config.preset, MixPreset::PreserveOriginal);
    assert_eq!(config.original_audio_gain_db, 0.0);
    assert_eq!(config.dubbing_master_gain_db, -1.0);
    assert_eq!(config.ducking_attenuation_db, -6.0);
}

#[test]
fn test_ducking_interval_merging() {
    let mut config = DubbingMixConfiguration::new("proj_1");
    config.ducking_attack_ms = 50;
    config.ducking_release_ms = 150;
    config.ducking_hold_ms = 100;
    config.ducking_attenuation_db = -12.0;

    // Two overlapping segments:
    // Seg 1: 500ms to 2500ms
    // Seg 2: 2200ms to 3700ms
    let segments = vec![
        DubbingMixSegment {
            segment_id: "s1".to_string(),
            track_id: "t1".to_string(),
            asset_id: "a1".to_string(),
            resolved_file_path: PathBuf::from("/tmp/1.wav"),
            timeline_start_us: 500_000,
            duration_us: 2_000_000,
            source_in_us: 0,
            source_out_us: 2_000_000,
            gain_db: 0.0,
            fade_in_ms: 0,
            fade_out_ms: 0,
            is_aligned: false,
        },
        DubbingMixSegment {
            segment_id: "s2".to_string(),
            track_id: "t2".to_string(),
            asset_id: "a2".to_string(),
            resolved_file_path: PathBuf::from("/tmp/2.wav"),
            timeline_start_us: 2_200_000,
            duration_us: 1_500_000,
            source_in_us: 0,
            source_out_us: 1_500_000,
            gain_db: 0.0,
            fade_in_ms: 0,
            fade_out_ms: 0,
            is_aligned: false,
        },
    ];

    let merged = compute_ducking_intervals(&segments, &config);
    assert_eq!(
        merged.len(),
        1,
        "Overlapping speech intervals must be merged into 1 continuous ducking envelope"
    );

    let duck = &merged[0];
    assert_eq!(duck.start_us, 500_000);
    // End should be speech end (3700ms) + hold (100ms) = 3800ms
    assert_eq!(duck.end_us, 3_800_000);
    assert_eq!(duck.attack_us, 50_000);
    assert_eq!(duck.release_us, 150_000);
    assert_eq!(duck.attenuation_db, -12.0);
}

#[test]
fn test_ducking_interval_separation_preserves_pauses() {
    let mut config = DubbingMixConfiguration::new("proj_1");
    config.ducking_attack_ms = 50;
    config.ducking_release_ms = 150;
    config.ducking_hold_ms = 100;
    config.ducking_attenuation_db = -12.0;

    // Two widely separated segments:
    // Seg 1: 1.0s to 2.0s
    // Seg 2: 6.0s to 7.0s
    let segments = vec![
        DubbingMixSegment {
            segment_id: "s1".to_string(),
            track_id: "t1".to_string(),
            asset_id: "a1".to_string(),
            resolved_file_path: PathBuf::from("/tmp/1.wav"),
            timeline_start_us: 1_000_000,
            duration_us: 1_000_000,
            source_in_us: 0,
            source_out_us: 1_000_000,
            gain_db: 0.0,
            fade_in_ms: 0,
            fade_out_ms: 0,
            is_aligned: false,
        },
        DubbingMixSegment {
            segment_id: "s2".to_string(),
            track_id: "t2".to_string(),
            asset_id: "a2".to_string(),
            resolved_file_path: PathBuf::from("/tmp/2.wav"),
            timeline_start_us: 6_000_000,
            duration_us: 1_000_000,
            source_in_us: 0,
            source_out_us: 1_000_000,
            gain_db: 0.0,
            fade_in_ms: 0,
            fade_out_ms: 0,
            is_aligned: false,
        },
    ];

    let merged = compute_ducking_intervals(&segments, &config);
    assert_eq!(
        merged.len(),
        2,
        "Separated speech intervals must not be merged prematurely"
    );
    assert_eq!(merged[0].start_us, 1_000_000);
    assert_eq!(merged[0].end_us, 2_100_000);
    assert_eq!(merged[1].start_us, 6_000_000);
    assert_eq!(merged[1].end_us, 7_100_000);
}

#[test]
fn test_ducking_volume_expression_generation() {
    let intervals = vec![DuckingInterval {
        start_us: 1_000_000,
        end_us: 3_000_000,
        attack_us: 100_000,
        release_us: 200_000,
        attenuation_db: -12.0,
    }];

    let expr = build_ducking_volume_expression(&intervals).expect("Expression should be generated");
    assert!(expr.contains("if(between(t,"));
    assert!(expr.contains("0.2512")); // -12 dB in linear is ~0.2512
}

#[test]
fn test_filter_graph_generation_multitrack_with_limiter() {
    let mut config = DubbingMixConfiguration::new("proj_mix_1");
    config.original_audio_enabled = true;
    config.ducking_enabled = true;
    config.ducking_attenuation_db = -12.0;

    let ducking_intervals = vec![DuckingInterval {
        start_us: 500_000,
        end_us: 3_800_000,
        attack_us: 50_000,
        release_us: 150_000,
        attenuation_db: -12.0,
    }];

    let segments = vec![
        DubbingMixSegment {
            segment_id: "seg_1".to_string(),
            track_id: "track_1".to_string(),
            asset_id: "asset_1".to_string(),
            resolved_file_path: PathBuf::from("/tmp/test_asset_1.wav"),
            timeline_start_us: 500_000,
            duration_us: 2_000_000,
            source_in_us: 0,
            source_out_us: 2_000_000,
            gain_db: 0.0,
            fade_in_ms: 20,
            fade_out_ms: 30,
            is_aligned: false,
        },
        DubbingMixSegment {
            segment_id: "seg_2".to_string(),
            track_id: "track_2".to_string(),
            asset_id: "asset_2".to_string(),
            resolved_file_path: PathBuf::from("/tmp/test_asset_2.wav"),
            timeline_start_us: 2_200_000,
            duration_us: 1_500_000,
            source_in_us: 0,
            source_out_us: 1_500_000,
            gain_db: 1.58,
            fade_in_ms: 10,
            fade_out_ms: 15,
            is_aligned: false,
        },
    ];

    let plan = DubbingMixPlan {
        plan_id: "plan_1".to_string(),
        project_id: "proj_mix_1".to_string(),
        timeline_duration_us: 4_000_000,
        has_original_audio: true,
        original_audio_path: Some(PathBuf::from("/tmp/video.mp4")),
        original_audio_sample_rate: 48000,
        original_audio_channels: 2,
        original_gain_db: -6.0,
        dubbing_master_gain_db: 0.0,
        segments,
        ducking_intervals,
        output_sample_rate: 48000,
        output_channels: 2,
        filter_graph_desc: String::new(),
        warnings: vec![],
    };

    let graph = build_complex_filter_graph(&plan);

    // Verify key filter graph components:
    assert!(
        graph.contains("aresample=48000"),
        "Must resample all streams to 48kHz"
    );
    assert!(
        graph.contains("aformat=channel_layouts=stereo"),
        "Must format to stereo"
    );
    assert!(
        graph.contains("amix=inputs=3:normalize=0"),
        "Must mix without auto-scaling normalization"
    );
    assert!(
        graph.contains("alimiter=limit=0.891"),
        "Must apply peak limiter with -1.0 dBFS ceiling"
    );
    assert!(
        graph.contains("adelay="),
        "Must apply sample-accurate adelay positioning"
    );
    assert!(
        graph.contains("afade="),
        "Must apply afade curves for segments"
    );
}

#[tokio::test]
async fn test_solo_and_mute_semantics_in_planner() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project = create_test_project(temp_dir.path());
    let mut config = DubbingMixConfiguration::new("proj_mix_1");
    config.original_audio_enabled = false;

    // Plan with both tracks unmuted, no solo
    let plan = plan_dubbing_mix(&project, &config, None)
        .await
        .expect("Planning should succeed");

    assert_eq!(plan.segments.len(), 2, "Both segments should be present");

    // Now mute Track 1
    let mut config_muted = config.clone();
    config_muted.track_settings.insert(
        "track_1".to_string(),
        DubbingMixTrackSettings {
            track_id: "track_1".to_string(),
            is_enabled: true,
            is_muted: true,
            is_solo: false,
            gain_db: 0.0,
            pan: 0.0,
        },
    );

    let plan_muted = plan_dubbing_mix(&project, &config_muted, None)
        .await
        .expect("Planning should succeed");

    assert_eq!(
        plan_muted.segments.len(),
        1,
        "Only track 2 should participate when track 1 is muted"
    );
    assert_eq!(plan_muted.segments[0].segment_id, "seg_2");

    // Now solo Track 1
    let mut config_solo = config.clone();
    config_solo.track_settings.insert(
        "track_1".to_string(),
        DubbingMixTrackSettings {
            track_id: "track_1".to_string(),
            is_enabled: true,
            is_muted: false,
            is_solo: true,
            gain_db: 0.0,
            pan: 0.0,
        },
    );

    let plan_solo = plan_dubbing_mix(&project, &config_solo, None)
        .await
        .expect("Planning should succeed");

    assert_eq!(
        plan_solo.segments.len(),
        1,
        "Only soloed track 1 should participate"
    );
    assert_eq!(plan_solo.segments[0].segment_id, "seg_1");
}

#[tokio::test]
async fn test_aligned_asset_resolution_prevents_double_alignment() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut project = create_test_project(temp_dir.path());

    let aligned_file = temp_dir.path().join("aligned_1.wav");
    fs::write(&aligned_file, b"RIFFaligned_1_data").unwrap();

    // Register an aligned asset for seg_1
    let aligned_asset = DubbingAudioAsset {
        asset_id: "asset_aligned_1".to_string(),
        file_path: aligned_file,
        file_name: "aligned_1.wav".to_string(),
        sha256: "hash_aligned_1_abcdef0123456789".to_string(),
        sample_rate: 22_050,
        channel_count: 1,
        duration_us: 1_900_000,
        size_bytes: 83_790,
        voice_id: "piper-en-lessac-medium".to_string(),
        speaker_id: None,
        created_at_epoch_ms: 3000,
    };
    project
        .assets
        .insert(aligned_asset.asset_id.clone(), aligned_asset);

    // Attach aligned asset to seg_1
    project.tracks[0].segments[0].aligned_audio_asset_id = Some("asset_aligned_1".to_string());
    project.tracks[0].segments[0].alignment_mode = Some(DubbingAlignmentMode::AutoFit);
    project.tracks[0].segments[0].alignment_result = Some(DubbingAlignmentResult {
        alignment_id: "align_res_1".to_string(),
        segment_id: "seg_1".to_string(),
        original_asset_id: "asset_1".to_string(),
        aligned_asset_id: Some("asset_aligned_1".to_string()),
        actual_output_duration_us: 1_900_000,
        effective_placement_start_us: 500_000,
        effective_placement_end_us: 2_400_000,
        processing_method: "atempo=1.05".to_string(),
        measured_duration_error_us: 0,
        tempo_factor_applied: 1.05,
        padding_before_applied_us: 0,
        padding_after_applied_us: 0,
        diagnostics: vec![],
        status: DubbingAlignmentStatus::Aligned,
        created_at_epoch_ms: 3000,
    });

    let config = DubbingMixConfiguration::new("proj_mix_1");

    let plan = plan_dubbing_mix(&project, &config, None)
        .await
        .expect("Planning should succeed");

    assert_eq!(plan.segments.len(), 2);
    let planned_seg_1 = &plan.segments[0];
    assert_eq!(
        planned_seg_1.asset_id, "asset_aligned_1",
        "Must resolve aligned audio asset when present"
    );
    assert!(
        planned_seg_1.is_aligned,
        "Must mark is_aligned as true to prevent double alignment or re-trimming"
    );
    assert_eq!(planned_seg_1.duration_us, 1_900_000);
}

#[test]
fn test_cache_key_determinism() {
    let mut config = DubbingMixConfiguration::new("proj_1");
    config.ducking_enabled = true;

    let plan_1 = DubbingMixPlan {
        plan_id: "plan_1".to_string(),
        project_id: "proj_1".to_string(),
        timeline_duration_us: 10_000_000,
        has_original_audio: true,
        original_audio_path: Some(PathBuf::from("/videos/intro.mp4")),
        original_audio_sample_rate: 48000,
        original_audio_channels: 2,
        original_gain_db: -6.0,
        dubbing_master_gain_db: 0.0,
        segments: vec![DubbingMixSegment {
            segment_id: "seg_1".to_string(),
            track_id: "track_1".to_string(),
            asset_id: "asset_1".to_string(),
            resolved_file_path: PathBuf::from("/assets/asset_1.wav"),
            timeline_start_us: 1_000_000,
            duration_us: 2_000_000,
            source_in_us: 0,
            source_out_us: 2_000_000,
            gain_db: 0.0,
            fade_in_ms: 10,
            fade_out_ms: 10,
            is_aligned: false,
        }],
        ducking_intervals: vec![],
        output_sample_rate: 48000,
        output_channels: 2,
        filter_graph_desc: "aresample=48000".to_string(),
        warnings: vec![],
    };

    let key_1 = compute_mix_plan_cache_key(&plan_1);
    let key_2 = compute_mix_plan_cache_key(&plan_1);
    assert_eq!(
        key_1, key_2,
        "Cache key calculation must be completely deterministic"
    );

    // Changing any parameter must invalidate the cache key
    let mut plan_changed = plan_1.clone();
    plan_changed.original_gain_db = -18.0;
    let key_3 = compute_mix_plan_cache_key(&plan_changed);
    assert_ne!(
        key_1, key_3,
        "Cache key must change when mixing configuration changes"
    );
}

#[test]
fn test_project_operations_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut project = create_test_project(temp_dir.path());
    assert!(project.mix_configuration.is_none());
    assert!(project.mix_result.is_none());

    let mut config = DubbingMixConfiguration::new("proj_mix_1");
    config.preset = MixPreset::VoiceFocus;
    config.original_audio_gain_db = -10.0;

    // Apply UpdateMixConfiguration operation
    let op_update = DubbingOperation::UpdateMixConfiguration {
        configuration: Box::new(config.clone()),
    };
    apply_operation(&mut project, &op_update).expect("Operation should succeed");
    assert!(project.mix_configuration.is_some());
    assert_eq!(
        project.mix_configuration.as_ref().unwrap().preset,
        MixPreset::VoiceFocus
    );

    // Apply ApplyMixResult operation
    let result = DubbingMixResult {
        result_id: "mix_res_1".to_string(),
        project_id: "proj_mix_1".to_string(),
        mix_asset_id: "mix_asset_1".to_string(),
        file_path: PathBuf::from("/tmp/mix.wav"),
        sha256: "abc123hash".to_string(),
        actual_duration_us: 4_000_000,
        sample_rate: 48000,
        channels: 2,
        bit_depth: 16,
        file_size_bytes: 384_000,
        peak_dbfs: -1.0,
        rms_dbfs: -18.5,
        integrated_lufs: Some(-19.0),
        clipping_samples: 0,
        warnings: vec![],
        created_at_epoch_ms: 4000,
    };

    let op_result = DubbingOperation::ApplyMixResult {
        result: Box::new(result.clone()),
    };
    apply_operation(&mut project, &op_result).expect("Operation should succeed");
    assert!(project.mix_result.is_some());
    assert_eq!(
        project.mix_result.as_ref().unwrap().mix_asset_id,
        "mix_asset_1"
    );

    // Apply InvalidateMixResult operation
    let op_invalidate = DubbingOperation::InvalidateMixResult;
    apply_operation(&mut project, &op_invalidate).expect("Operation should succeed");
    assert!(
        project.mix_result.is_none(),
        "Mix result should be cleared after invalidation"
    );
    // Mix configuration should still be retained
    assert!(project.mix_configuration.is_some());
}
