use std::fs::File;
use std::io::Write;
use std::path::Path;

use opentake_core::editing::time::MediaTimeUs;
use opentake_core::localization::dubbing::history::DubbingHistory;
use opentake_core::localization::dubbing::models::*;
use opentake_core::localization::dubbing::operations::{apply_operation, DubbingOperation};
use opentake_core::localization::dubbing::persistence::*;
use opentake_core::localization::dubbing::segments::*;
use opentake_core::localization::dubbing::timeline::*;
use opentake_core::localization::dubbing::tracks::*;
use opentake_core::localization::dubbing::validation::validate_dubbing_project;
use opentake_core::localization::dubbing::waveforms::generate_waveform_from_wav;
use opentake_core::localization::subtitles::model::{
    SubtitleCue, SubtitleDocument, SubtitleFormat, SubtitleTrack,
};

fn write_dummy_wav(path: &Path, sample_rate: u32, duration_secs: f32) {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let data_len = (num_samples * 2) as u32;
    let total_len = 36 + data_len;

    let mut file = File::create(path).expect("failed to create dummy wav");
    file.write_all(b"RIFF").unwrap();
    file.write_all(&total_len.to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    file.write_all(&1u16.to_le_bytes()).unwrap(); // 1 channel
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap(); // byte rate
    file.write_all(&2u16.to_le_bytes()).unwrap(); // block align
    file.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample
    file.write_all(b"data").unwrap();
    file.write_all(&data_len.to_le_bytes()).unwrap();

    for i in 0..num_samples {
        let sample = ((i as f32 * 0.1).sin() * 16000.0) as i16;
        file.write_all(&sample.to_le_bytes()).unwrap();
    }
}

#[test]
fn test_dubbing_project_creation_and_track_management() {
    let mut doc = SubtitleDocument::new(Some("media_asset_1".to_string()));
    let mut track = SubtitleTrack::new("track_es", "es", "Spanish Subtitles", SubtitleFormat::Srt);
    track.cues.push(
        SubtitleCue::new(
            "cue_1",
            MediaTimeUs(1_000_000),
            MediaTimeUs(3_000_000),
            "Hola mundo",
        )
        .unwrap(),
    );
    doc.tracks.push(track);

    let mut project = create_project_from_subtitles(&doc, "track_es", Some("es".to_string()))
        .expect("should create dubbing project");

    assert_eq!(project.source_language, "es");
    assert_eq!(project.target_language, "es");
    assert_eq!(project.media_reference.as_deref(), Some("media_asset_1"));
    assert_eq!(project.tracks.len(), 1);

    let dub_track = &project.tracks[0];
    assert_eq!(
        dub_track.source_subtitle_track_id.as_deref(),
        Some("track_es")
    );
    assert_eq!(dub_track.language, "es");

    // Add track
    let track_id = add_track(
        &mut project,
        "Director Notes".to_string(),
        "es".to_string(),
        None,
        None,
        None,
    );
    assert_eq!(project.tracks.len(), 2);

    // Mute and Solo
    set_track_mute(&mut project, &track_id, true).unwrap();
    assert!(project.find_track(&track_id).unwrap().is_muted);

    set_track_solo(&mut project, &track_id, true).unwrap();
    assert!(project.find_track(&track_id).unwrap().is_solo);

    // Reorder tracks
    let reversed_ids = vec![project.tracks[1].id.clone(), project.tracks[0].id.clone()];
    reorder_tracks(&mut project, &reversed_ids).unwrap();
    assert_eq!(project.tracks[0].id, track_id);

    // Remove track
    remove_track(&mut project, &track_id).unwrap();
    assert_eq!(project.tracks.len(), 1);
}

#[test]
fn test_segment_non_destructive_operations() {
    let mut project = DubbingProject::new(
        "sub_doc_1".to_string(),
        "en".to_string(),
        "en".to_string(),
        Some("media_1".to_string()),
    );
    let track_id = "track_1".to_string();
    project.tracks.push(DubbingTrack::new(
        track_id.clone(),
        "English Dub".to_string(),
        "en".to_string(),
    ));

    // Register dummy asset of 4.0 seconds (4,000,000 us)
    let asset_id = "asset_speech_1".to_string();
    let temp_dir = std::env::temp_dir().join("omw_dubbing_tests");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let wav_path = temp_dir.join("test_segment.wav");
    write_dummy_wav(&wav_path, 22050, 4.0);

    project.assets.insert(
        asset_id.clone(),
        DubbingAudioAsset {
            asset_id: asset_id.clone(),
            file_path: wav_path.clone(),
            file_name: "test_segment.wav".to_string(),
            sha256: "test_hash".to_string(),
            sample_rate: 22050,
            channel_count: 1,
            duration_us: 4_000_000,
            size_bytes: 176444,
            voice_id: "voice_1".to_string(),
            speaker_id: None,
            created_at_epoch_ms: 1000,
        },
    );

    let seg = DubbingSegment {
        id: "seg_1".to_string(),
        track_id: track_id.clone(),
        source_subtitle_cue_id: Some("cue_1".to_string()),
        source_subtitle_track_id: None,
        source_text: "Welcome to Open Media Workbench".to_string(),
        source_text_hash: "hash_1".to_string(),
        voice_id: "voice_1".to_string(),
        speaker_id: None,
        audio_asset_id: asset_id.clone(),
        source_in_us: 0,
        source_out_us: 4_000_000,
        timeline_start_us: 2_000_000,
        original_cue_start_us: Some(2_000_000),
        original_cue_end_us: Some(4_500_000),
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: None,
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    };
    project.tracks[0].segments.push(seg);

    // Initial timings
    let seg = &project.tracks[0].segments[0];
    assert_eq!(seg.trimmed_duration_us(), 4_000_000);
    assert_eq!(seg.timeline_end_us(), 6_000_000);
    // Subtitle interval is 2.5s (4.5s - 2.0s = 2,500,000 us). Audio is 4.0s. Overrun is 1.5s = 1,500,000 us.
    assert_eq!(seg.cue_overrun_us(), Some(1_500_000));

    // A. MOVE
    move_segment(&mut project, "seg_1", 3_000_000).unwrap();
    assert_eq!(project.tracks[0].segments[0].timeline_start_us, 3_000_000);
    assert_eq!(project.tracks[0].segments[0].timeline_end_us(), 7_000_000);

    // B. TRIM START (Advance in-point by 500ms; head moves forward by 500ms, tail stays at 7s)
    trim_segment_start(&mut project, "seg_1", 500_000).unwrap();
    let seg = &project.tracks[0].segments[0];
    assert_eq!(seg.source_in_us, 500_000);
    assert_eq!(seg.trimmed_duration_us(), 3_500_000);
    assert_eq!(seg.timeline_start_us, 3_500_000);
    assert_eq!(seg.timeline_end_us(), 7_000_000);

    // C. TRIM END (Cut 500ms from tail)
    trim_segment_end(&mut project, "seg_1", 3_500_000).unwrap();
    let seg = &project.tracks[0].segments[0];
    assert_eq!(seg.source_out_us, 3_500_000);
    assert_eq!(seg.trimmed_duration_us(), 3_000_000);
    assert_eq!(seg.timeline_start_us, 3_500_000);
    assert_eq!(seg.timeline_end_us(), 6_500_000);

    // D. SPLIT (Split at timeline position 5_000_000 us, which is 1.5s into the 3.0s trimmed segment)
    let (left_id, right_id) = split_segment(&mut project, "seg_1", 5_000_000).unwrap();
    assert_eq!(project.tracks[0].segments.len(), 2);
    let left = project.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == left_id)
        .unwrap();
    let right = project.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == right_id)
        .unwrap();

    // Source asset unchanged and shared!
    assert_eq!(left.audio_asset_id, asset_id);
    assert_eq!(right.audio_asset_id, asset_id);

    // Left segment: starts at 3_500_000, duration 1_500_000 (timeline end 5_000_000)
    assert_eq!(left.timeline_start_us, 3_500_000);
    assert_eq!(left.source_in_us, 500_000);
    assert_eq!(left.source_out_us, 2_000_000);
    assert_eq!(left.trimmed_duration_us(), 1_500_000);
    assert_eq!(left.timeline_end_us(), 5_000_000);

    // Right segment: starts at 5_000_000, duration 1_500_000 (timeline end 6_500_000)
    assert_eq!(right.timeline_start_us, 5_000_000);
    assert_eq!(right.source_in_us, 2_000_000);
    assert_eq!(right.source_out_us, 3_500_000);
    assert_eq!(right.trimmed_duration_us(), 1_500_000);
    assert_eq!(right.timeline_end_us(), 6_500_000);

    // E. DUPLICATE
    let dup_id = duplicate_segment(&mut project, &left_id, 10_000_000).unwrap();
    let dup = project.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == dup_id)
        .unwrap();
    assert_eq!(dup.timeline_start_us, 13_500_000);
    assert_eq!(dup.audio_asset_id, asset_id);
    assert_eq!(dup.trimmed_duration_us(), 1_500_000);

    // F. DELETE
    delete_segment(&mut project, &dup_id).unwrap();
    assert!(project.tracks[0].segments.iter().all(|s| s.id != dup_id));

    // G. GAIN
    set_segment_gain(&mut project, &left_id, 1.25).unwrap();
    let left = project.tracks[0]
        .segments
        .iter()
        .find(|s| s.id == left_id)
        .unwrap();
    assert!((left.gain - 1.25).abs() < f32::EPSILON);
}

#[test]
fn test_waveform_peak_generation_from_real_wav() {
    let temp_dir = std::env::temp_dir().join("omw_dubbing_tests");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let wav_path = temp_dir.join("waveform_test.wav");
    write_dummy_wav(&wav_path, 22050, 2.0);

    let wf = generate_waveform_from_wav(&wav_path, "seg_wf", "asset_wf")
        .expect("should generate waveform");

    assert_eq!(wf.segment_id, "seg_wf");
    assert_eq!(wf.asset_id, "asset_wf");
    assert_eq!(wf.sample_rate, 22050);
    assert_eq!(wf.duration_us, 2_000_000);
    assert!(!wf.bins.is_empty());

    // Check bounds on bins
    for bin in &wf.bins {
        assert!(bin.min >= -1.0 && bin.min <= 1.0);
        assert!(bin.max >= -1.0 && bin.max <= 1.0);
        assert!(bin.rms >= 0.0 && bin.rms <= 1.0);
        assert!(bin.max >= bin.min);
    }
}

#[test]
fn test_dubbing_diagnostics_detection() {
    let mut project = DubbingProject::new(
        "doc_diag".to_string(),
        "en".to_string(),
        "en".to_string(),
        None,
    );
    let track_id = "track_diag".to_string();
    let mut track = DubbingTrack::new(track_id.clone(), "Diag Track".to_string(), "en".to_string());
    track.assigned_voice_id = None; // Unresolved voice!

    // Segment 1: Exceeds subtitle cue and has missing audio asset
    track.segments.push(DubbingSegment {
        id: "s1".to_string(),
        track_id: track_id.clone(),
        source_subtitle_cue_id: Some("cue_1".to_string()),
        source_subtitle_track_id: None,
        source_text: "Long speech".to_string(),
        source_text_hash: "hash".to_string(),
        voice_id: "voice_missing".to_string(),
        speaker_id: None,
        audio_asset_id: "non_existent_asset".to_string(),
        source_in_us: 0,
        source_out_us: 5_000_000,
        timeline_start_us: 1_000_000,
        original_cue_start_us: Some(1_000_000),
        original_cue_end_us: Some(3_000_000), // cue is 2s, audio is 5s => overrun of 3s
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: None,
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    });

    // Segment 2: Overlaps segment 1 (starts at 4_000_000 while s1 ends at 6_000_000)
    track.segments.push(DubbingSegment {
        id: "s2".to_string(),
        track_id: track_id.clone(),
        source_subtitle_cue_id: Some("cue_2".to_string()),
        source_subtitle_track_id: None,
        source_text: "Overlap speech".to_string(),
        source_text_hash: "hash2".to_string(),
        voice_id: "voice_missing".to_string(),
        speaker_id: None,
        audio_asset_id: "non_existent_asset".to_string(),
        source_in_us: 0,
        source_out_us: 2_000_000,
        timeline_start_us: 4_000_000,
        original_cue_start_us: Some(4_000_000),
        original_cue_end_us: Some(6_000_000),
        gain: 1.0,
        status: DubbingSegmentStatus::Ready,
        warnings: Vec::new(),
        original_audio_asset_id: None,
        aligned_audio_asset_id: None,
        alignment_mode: None,
        alignment_plan: None,
        alignment_result: None,
        start_offset_us: None,
    });

    project.tracks.push(track);

    let findings = validate_dubbing_project(&project, None);
    let codes: Vec<DubbingDiagnosticCode> = findings.iter().map(|f| f.code).collect();

    assert!(codes.contains(&DubbingDiagnosticCode::UnresolvedVoiceAssignment));
    assert!(codes.contains(&DubbingDiagnosticCode::MissingAudioAsset));
    assert!(codes.contains(&DubbingDiagnosticCode::SegmentExceedsSubtitleInterval));
    assert!(codes.contains(&DubbingDiagnosticCode::SegmentOverlap));
}

#[test]
fn test_undo_redo_history() {
    let mut history = DubbingHistory::new();
    let mut project = DubbingProject::new(
        "doc_hist".to_string(),
        "en".to_string(),
        "en".to_string(),
        None,
    );
    let track_id = "track_hist".to_string();
    project.tracks.push(DubbingTrack::new(
        track_id.clone(),
        "Hist Track".to_string(),
        "en".to_string(),
    ));

    // Record initial snapshot
    history.record_snapshot(&project);

    // Apply mute operation
    let op = DubbingOperation::SetTrackMute {
        track_id: track_id.clone(),
        is_muted: true,
    };
    apply_operation(&mut project, &op).unwrap();
    assert!(project.find_track(&track_id).unwrap().is_muted);
    assert!(history.can_undo());
    assert!(!history.can_redo());

    // Undo
    let undone = history.undo(project.clone()).expect("undo should succeed");
    assert!(!undone.find_track(&track_id).unwrap().is_muted);
    assert!(!history.can_undo());
    assert!(history.can_redo());

    // Redo
    let redone = history.redo(undone).expect("redo should succeed");
    assert!(redone.find_track(&track_id).unwrap().is_muted);
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn test_project_persistence_standalone_and_document_embed() {
    let mut project = DubbingProject::new(
        "doc_persist".to_string(),
        "en".to_string(),
        "es".to_string(),
        Some("media_persist_1".to_string()),
    );
    project.tracks.push(DubbingTrack::new(
        "track_es".to_string(),
        "Spanish Dub".to_string(),
        "es".to_string(),
    ));

    // 1. Test standalone file save/load
    let temp_dir = std::env::temp_dir().join("omw_dubbing_tests");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let json_file = temp_dir.join("project.dub.json");

    save_dubbing_project_to_file(&project, &json_file).expect("should save to file");
    let loaded = load_dubbing_project_from_file(&json_file).expect("should load from file");

    assert_eq!(loaded.id, project.id);
    assert_eq!(loaded.target_language, "es");
    assert_eq!(loaded.tracks.len(), 1);

    // 2. Test SubtitleDocument metadata embed & roundtrip
    let mut doc = SubtitleDocument::new(Some("media_persist_1".to_string()));
    save_dubbing_project_to_document(&mut doc, &project).expect("should embed into document");

    assert!(doc.metadata.contains_key(DUBBING_METADATA_KEY));
    let embedded = load_dubbing_project_from_document(&doc).expect("should load from metadata");
    let embedded_proj = embedded.expect("dubbing project should be present");

    assert_eq!(embedded_proj.id, project.id);
    assert_eq!(embedded_proj.target_language, "es");
}
