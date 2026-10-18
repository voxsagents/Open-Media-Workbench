use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use opentake_core::ai::catalog::find_catalog_model;
use opentake_core::ai::models::AiModelInstallation;
use opentake_core::ai::storage::AiStorageManager;
use opentake_core::ai::tts::audio::validate_wav_file;
use opentake_core::ai::tts::engine::SystemPiperCliEngine;
use opentake_core::editing::time::MediaTimeUs;
use opentake_core::localization::dubbing::alignment::{
    AlignmentConfig, DubbingAlignmentMode, DubbingAlignmentStatus, SilencePaddingMode,
};
use opentake_core::localization::dubbing::operations::{apply_operation, DubbingOperation};
use opentake_core::localization::dubbing::persistence::{
    load_dubbing_project_from_file, save_dubbing_project_to_file,
};
use opentake_core::localization::dubbing::DubbingManager;
use opentake_core::localization::subtitles::model::{
    SubtitleCue, SubtitleDocument, SubtitleFormat, SubtitleTrack,
};

#[tokio::test]
async fn test_real_ffmpeg_dubbing_alignment_e2e() {
    let home = std::env::var("HOME").unwrap_or_default();
    let piper_path = PathBuf::from(&home).join(".local/share/opentake/bin/piper");
    let real_installed_dir =
        PathBuf::from(&home).join(".local/share/opentake/models/installed/piper-en-lessac-medium");
    let real_model = real_installed_dir.join("en_US-lessac-medium.onnx");
    let real_config = real_installed_dir.join("en_US-lessac-medium.onnx.json");

    if !piper_path.exists() || !real_model.exists() || !real_config.exists() {
        eprintln!("Piper or model not found, skipping real alignment E2E test");
        return;
    }

    let temp_root = tempdir().unwrap();
    let models_dir = temp_root.path().join("models");
    let storage = Arc::new(AiStorageManager::new(&models_dir).unwrap());
    let engine = Arc::new(SystemPiperCliEngine::new(Some(piper_path)));

    // Commit real model into test storage
    let manifest = find_catalog_model("piper-en-lessac-medium").expect("catalog model found");
    let model_dir = storage.resolve_model_dir(&manifest.id).unwrap();
    fs::create_dir_all(&model_dir).unwrap();
    let staging_model = temp_root.path().join("staging_piper.onnx");
    let config_dest = model_dir.join("en_US-lessac-medium.onnx.json");

    if fs::hard_link(&real_model, &staging_model).is_err() {
        fs::copy(&real_model, &staging_model).unwrap();
    }
    if fs::hard_link(&real_config, &config_dest).is_err() {
        fs::copy(&real_config, &config_dest).unwrap();
    }

    let actual_size = fs::metadata(&staging_model).map(|m| m.len()).unwrap_or(32);
    let installation = AiModelInstallation {
        model_id: manifest.id.clone(),
        installed_at: "2026-10-18T00:00:00Z".to_string(),
        installed_version: manifest.version.clone(),
        artifact_filename: manifest.artifact_filename.clone(),
        actual_size_bytes: actual_size,
        verified_sha256: manifest.expected_sha256.clone().unwrap_or_default(),
        installed_artifacts: vec![],
        in_use: false,
    };
    storage
        .commit_installation(&staging_model, installation)
        .unwrap();

    let manager = DubbingManager::new(storage.clone(), engine, temp_root.path().to_path_buf());

    // Setup 3 subtitle cues designed to test Case A, Case B, Case C:
    // Cue 1 (Case A: Speech longer than cue window)
    // "Welcome to Open Media Workbench." takes ~2.2s speech, place in 2.0s window -> requires ~1.10x tempo
    // Cue 2 (Case B: Speech shorter than cue window)
    // "Short text." takes ~0.9s speech, place in 2.5s window -> requires silence padding
    // Cue 3 (Case C: Speech far too long for safe AutoFit)
    // "This is a very comprehensive demonstration of offline speech duration alignment with artificial intelligence." takes ~6.5s, place in 2.0s window -> requires > 3.0x tempo -> NeedsReview!
    let mut doc = SubtitleDocument::new(Some("sample_video_asset".to_string()));
    let mut sub_track = SubtitleTrack::new(
        "sub_track_align",
        "en",
        "English Dialogue",
        SubtitleFormat::Srt,
    );

    let cue_1 = SubtitleCue::new(
        "cue_1",
        MediaTimeUs(1_000_000), // 1.0s to 3.0s (2.0s window)
        MediaTimeUs(3_000_000),
        "Welcome to Open Media Workbench.",
    )
    .unwrap();

    let cue_2 = SubtitleCue::new(
        "cue_2",
        MediaTimeUs(4_000_000), // 4.0s to 6.5s (2.5s window)
        MediaTimeUs(6_500_000),
        "Short text.",
    )
    .unwrap();

    let cue_3 = SubtitleCue::new(
        "cue_3",
        MediaTimeUs(7_000_000), // 7.0s to 9.0s (2.0s window)
        MediaTimeUs(9_000_000),
        "This is a very comprehensive demonstration of offline speech duration alignment with artificial intelligence and local machine learning models.",
    )
    .unwrap();

    sub_track.cues.push(cue_1);
    sub_track.cues.push(cue_2);
    sub_track.cues.push(cue_3);
    doc.tracks.push(sub_track);

    // Create dubbing project and synthesize cues
    let mut project = manager
        .create_project(&doc, "sub_track_align", Some("en".to_string()))
        .unwrap();

    let dubbing_track_id = project.tracks[0].id.clone();
    let created_seg_ids = manager
        .synthesize_track_cues(
            &mut project,
            &dubbing_track_id,
            &doc,
            None,
            Some("piper-en-lessac-medium".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(created_seg_ids.len(), 3);

    let config = AlignmentConfig {
        min_auto_tempo: 0.85,
        max_auto_tempo: 1.25, // allow up to 1.25x for testing AutoFit
        min_supported_tempo: 0.50,
        max_supported_tempo: 2.00,
        tolerance_us: 20_000,
        default_padding_mode: SilencePaddingMode::PadEnd,
        algorithm_version: 1,
    };

    // ==========================================
    // CASE A: Speech longer than cue window
    // ==========================================
    let seg_1_id = &created_seg_ids[0];
    let orig_asset_1 = {
        let seg = project.find_segment(seg_1_id).unwrap().1;
        project.assets.get(&seg.audio_asset_id).unwrap().clone()
    };
    let orig_sha256_1 = orig_asset_1.sha256.clone();

    let plan_1 = manager
        .plan_segment_alignment(&project, seg_1_id, Some(&config), None, None, None)
        .unwrap();

    assert!(plan_1.proposed_tempo > 1.0, "Case A requires speedup");
    assert!(
        plan_1.proposed_tempo <= 1.25,
        "Case A tempo should be within max auto tempo"
    );
    assert!(!plan_1.requires_manual_review);

    // Execute real FFmpeg atempo processing
    let cancel = CancellationToken::new();
    let (result_1, aligned_asset_1) = manager
        .process_segment_alignment(&project, &plan_1, cancel.clone())
        .await
        .unwrap();

    assert_eq!(result_1.status, DubbingAlignmentStatus::Aligned);
    assert!(aligned_asset_1.file_path.exists());
    let validated_1 = validate_wav_file(&aligned_asset_1.file_path).unwrap();
    assert_eq!(validated_1.sample_rate, 22_050);

    // Verify duration error is within sample-aware tolerance (< 40ms)
    let dur_error =
        (result_1.actual_output_duration_us as i64) - (plan_1.target_duration_us as i64);
    assert!(
        dur_error.abs() < 45_000,
        "Actual duration error should be within 45ms, got: {} us",
        dur_error
    );

    // Confirm original WAV file is byte-identical (SHA-256 unchanged)
    let cur_orig_bytes = fs::read(&orig_asset_1.file_path).unwrap();
    let mut hasher = sha2::Sha256::default();
    sha2::Digest::update(&mut hasher, &cur_orig_bytes);
    let cur_sha = format!("{:x}", sha2::Digest::finalize(hasher));
    assert_eq!(
        cur_sha, orig_sha256_1,
        "Original WAV file must be immutable"
    );

    // Apply alignment operation to timeline
    let op_apply_1 = DubbingOperation::ApplyAlignment {
        segment_id: seg_1_id.clone(),
        plan: plan_1.clone(),
        result: result_1.clone(),
        aligned_asset: aligned_asset_1.clone(),
    };
    apply_operation(&mut project, &op_apply_1).unwrap();

    let applied_seg_1 = project.find_segment(seg_1_id).unwrap().1;
    assert_eq!(applied_seg_1.audio_asset_id, aligned_asset_1.asset_id);
    assert!(applied_seg_1.is_aligned());

    // ==========================================
    // CASE B: Speech shorter than cue window (Silence padding)
    // ==========================================
    let seg_2_id = &created_seg_ids[1];
    let plan_2 = manager
        .plan_segment_alignment(&project, seg_2_id, Some(&config), None, None, None)
        .unwrap();

    assert_eq!(
        plan_2.proposed_tempo, 1.0,
        "Short speech keeps original tempo"
    );
    assert!(
        plan_2.padding_after_us > 0,
        "Silence padding applied after dialogue"
    );
    assert_eq!(plan_2.padding_mode, SilencePaddingMode::PadEnd);
    assert!(!plan_2.requires_manual_review);

    let (result_2, aligned_asset_2) = manager
        .process_segment_alignment(&project, &plan_2, cancel.clone())
        .await
        .unwrap();

    assert_eq!(result_2.status, DubbingAlignmentStatus::Aligned);
    let validated_2 = validate_wav_file(&aligned_asset_2.file_path).unwrap();
    // Verify exact sample-aware duration match
    assert_eq!(validated_2.duration_us, plan_2.target_duration_us);

    // Apply Case B alignment
    let op_apply_2 = DubbingOperation::ApplyAlignment {
        segment_id: seg_2_id.clone(),
        plan: plan_2,
        result: result_2,
        aligned_asset: aligned_asset_2,
    };
    apply_operation(&mut project, &op_apply_2).unwrap();

    // ==========================================
    // CASE C: Speech far too long for safe AutoFit
    // ==========================================
    let seg_3_id = &created_seg_ids[2];
    let plan_3 = manager
        .plan_segment_alignment(&project, seg_3_id, Some(&config), None, None, None)
        .unwrap();

    assert!(
        plan_3.requires_manual_review,
        "Extreme overrun must require review"
    );
    assert!(plan_3.warnings.iter().any(|w| w.contains("exceeds")));

    // ==========================================
    // Project Persistence & Undo/Redo Roundtrip
    // ==========================================
    let project_file = temp_root.path().join("saved_dubbing_project.json");
    save_dubbing_project_to_file(&project, &project_file).unwrap();

    let reloaded_project = load_dubbing_project_from_file(&project_file).unwrap();
    let reloaded_seg_1 = reloaded_project.find_segment(seg_1_id).unwrap().1;
    assert_eq!(reloaded_seg_1.audio_asset_id, aligned_asset_1.asset_id);
    assert_eq!(
        reloaded_seg_1.aligned_audio_asset_id.as_deref(),
        Some(aligned_asset_1.asset_id.as_str())
    );
    assert_eq!(
        reloaded_seg_1.original_audio_asset_id.as_deref(),
        Some(orig_asset_1.asset_id.as_str())
    );
    assert!(reloaded_seg_1.is_aligned());

    // Restore original audio on Segment 1
    let op_restore = DubbingOperation::RestoreOriginalAudio {
        segment_id: seg_1_id.clone(),
    };
    apply_operation(&mut project, &op_restore).unwrap();

    let restored_seg_1 = project.find_segment(seg_1_id).unwrap().1;
    assert_eq!(restored_seg_1.audio_asset_id, orig_asset_1.asset_id);
    assert_eq!(
        restored_seg_1.alignment_mode,
        Some(DubbingAlignmentMode::Original)
    );
    assert!(!restored_seg_1.is_aligned());
}
