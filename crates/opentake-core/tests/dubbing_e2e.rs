use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

use opentake_core::ai::catalog::find_catalog_model;
use opentake_core::ai::models::AiModelInstallation;
use opentake_core::ai::storage::AiStorageManager;
use opentake_core::ai::tts::engine::SystemPiperCliEngine;
use opentake_core::editing::time::MediaTimeUs;
use opentake_core::localization::dubbing::persistence::*;
use opentake_core::localization::dubbing::DubbingManager;
use opentake_core::localization::subtitles::model::{
    SubtitleCue, SubtitleDocument, SubtitleFormat, SubtitleTrack,
};

#[tokio::test]
async fn test_real_piper_tts_to_dubbing_timeline_e2e() {
    let home = std::env::var("HOME").unwrap_or_default();
    let piper_path = PathBuf::from(&home).join(".local/share/opentake/bin/piper");
    let real_installed_dir =
        PathBuf::from(&home).join(".local/share/opentake/models/installed/piper-en-lessac-medium");
    let real_model = real_installed_dir.join("en_US-lessac-medium.onnx");
    let real_config = real_installed_dir.join("en_US-lessac-medium.onnx.json");

    if !piper_path.exists() || !real_model.exists() || !real_config.exists() {
        eprintln!("Piper or model not found in ~/.local/share/opentake, skipping real E2E test");
        return;
    }

    let temp_root = tempdir().unwrap();
    let models_dir = temp_root.path().join("models");
    let storage = Arc::new(AiStorageManager::new(&models_dir).unwrap());
    let engine = Arc::new(SystemPiperCliEngine::new(Some(piper_path)));

    // Commit model installation into clean test environment
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

    // Create DubbingManager with base_dir in temp_root so created WAVs stay in temp
    let manager = DubbingManager::new(storage.clone(), engine, temp_root.path().to_path_buf());

    // 1. Setup realistic English subtitle track with 3 cues
    let mut doc = SubtitleDocument::new(Some("sample_video_asset_123".to_string()));
    let mut sub_track = SubtitleTrack::new(
        "sub_track_en",
        "en",
        "English Dialogue",
        SubtitleFormat::Srt,
    );
    let cue1 = SubtitleCue::new(
        "cue_1",
        MediaTimeUs(1_000_000), // 1.0s
        MediaTimeUs(3_000_000), // 3.0s
        "Welcome to Open Media Workbench.",
    )
    .unwrap();
    let cue2 = SubtitleCue::new(
        "cue_2",
        MediaTimeUs(4_000_000), // 4.0s
        MediaTimeUs(6_000_000), // 6.0s
        "Create subtitles and translate videos.",
    )
    .unwrap();
    let cue3 = SubtitleCue::new(
        "cue_3",
        MediaTimeUs(7_000_000), // 7.0s
        MediaTimeUs(9_000_000), // 9.0s
        "Generate offline speech with AI.",
    )
    .unwrap();

    sub_track.cues.push(cue1.clone());
    sub_track.cues.push(cue2.clone());
    sub_track.cues.push(cue3.clone());
    doc.tracks.push(sub_track);

    // 2. Create Dubbing project from subtitles
    let mut project = manager
        .create_project(&doc, "sub_track_en", Some("en".to_string()))
        .expect("Project creation failed");

    assert_eq!(project.tracks.len(), 1);
    let dub_track_id = project.tracks[0].id.clone();

    // 3. Synthesize speech for cues via real Piper offline TTS engine
    let created_segment_ids = manager
        .synthesize_track_cues(
            &mut project,
            &dub_track_id,
            &doc,
            None,
            Some("piper-en-lessac-medium".to_string()),
            Some(0),
            None,
        )
        .await
        .expect("Real Piper synthesis and timeline placement failed");

    assert_eq!(created_segment_ids.len(), 3);
    assert_eq!(project.tracks[0].segments.len(), 3);
    assert_eq!(project.assets.len(), 3);

    // 4. Verify placements and audio metadata
    let (seg1_id, seg2_id, seg3_id, seg3_source_out) = {
        let seg1 = &project.tracks[0].segments[0];
        let seg2 = &project.tracks[0].segments[1];
        let seg3 = &project.tracks[0].segments[2];

        assert_eq!(seg1.timeline_start_us, 1_000_000); // 1.0s cue start
        assert_eq!(seg2.timeline_start_us, 4_000_000); // 4.0s cue start
        assert_eq!(seg3.timeline_start_us, 7_000_000); // 7.0s cue start

        // Check real WAV assets on disk
        for seg in [seg1, seg2, seg3] {
            let asset = project
                .assets
                .get(&seg.audio_asset_id)
                .expect("Asset missing from manifest");
            assert!(asset.file_path.exists(), "Audio file must exist on disk");
            assert_eq!(asset.sample_rate, 22050);
            assert_eq!(asset.channel_count, 1);
            assert!(asset.duration_us > 1_000_000, "Duration must be measurable");
            assert!(
                asset.size_bytes > 40_000,
                "File size must reflect real PCM WAV"
            );
            assert!(!asset.sha256.is_empty());
        }

        (
            seg1.id.clone(),
            seg2.id.clone(),
            seg3.id.clone(),
            seg3.source_out_us,
        )
    };

    // 5. Generate real waveforms from real audio
    let wf1 = manager
        .get_segment_waveform(&project, &seg1_id)
        .expect("Waveform generation failed");
    assert!(!wf1.bins.is_empty());
    assert_eq!(wf1.sample_rate, 22050);

    // 6. Test timeline diagnostics on real placed segments
    let findings = manager.get_diagnostics(&project, Some(&doc));
    // Verify diagnostics ran without crashing
    assert!(findings.iter().all(|f| !f.message.is_empty()));

    // 7. Non-destructive Move, Trim, Split
    use opentake_core::localization::dubbing::operations::DubbingOperation;
    manager
        .execute_operation(
            &mut project,
            &DubbingOperation::MoveSegment {
                segment_id: seg2_id.clone(),
                new_timeline_start_us: 4_500_000,
            },
        )
        .await
        .unwrap();

    let moved_seg2 = project.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == seg2_id)
        .unwrap();
    assert_eq!(moved_seg2.timeline_start_us, 4_500_000);

    // Trim segment 3
    manager
        .execute_operation(
            &mut project,
            &DubbingOperation::TrimSegmentEnd {
                segment_id: seg3_id.clone(),
                new_source_out_us: seg3_source_out.saturating_sub(200_000),
            },
        )
        .await
        .unwrap();

    // 8. Undo / Redo
    let undone = manager.undo(&project).await.expect("Undo should succeed");
    let undone_seg3 = undone.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == seg3_id)
        .unwrap();
    assert_eq!(undone_seg3.source_out_us, seg3_source_out);

    let redone = manager.redo(&undone).await.expect("Redo should succeed");
    let redone_seg3 = redone.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == seg3_id)
        .unwrap();
    assert_eq!(
        redone_seg3.source_out_us,
        seg3_source_out.saturating_sub(200_000)
    );

    // 9. Save and Reopen project
    let save_file = temp_root.path().join("saved_dubbing.json");
    save_dubbing_project_to_file(&redone, &save_file).unwrap();

    let reloaded = load_dubbing_project_from_file(&save_file).unwrap();
    assert_eq!(reloaded.id, redone.id);
    assert_eq!(reloaded.tracks[0].segments.len(), 3);
    assert_eq!(reloaded.assets.len(), 3);

    // Verify all referenced audio files still exist on disk after reload
    for asset in reloaded.assets.values() {
        assert!(asset.file_path.exists());
    }

    // 10. Verify original media and subtitle document are 100% immutable
    assert_eq!(doc.tracks[0].cues.len(), 3);
    assert_eq!(doc.tracks[0].cues[0].start_us, MediaTimeUs(1_000_000));
    assert_eq!(doc.tracks[0].cues[0].end_us, MediaTimeUs(3_000_000));
    assert_eq!(
        doc.tracks[0].cues[0].text,
        "Welcome to Open Media Workbench."
    );
}
