pub mod commands;
pub mod protocol;

use std::sync::Arc;

pub use commands::{
    cancel_media_diagnostics, cancel_media_recovery, cancel_preview_proxy,
    cancel_waveform_analysis, clear_media_library, export_diagnostic_report,
    export_recovery_report, finalize_recovered_asset, get_app_info, get_diagnostic_report,
    get_media_engine_status, get_media_library_assets, get_media_playback_url, get_proxy_status,
    get_recovery_status, get_waveform_segment, get_waveform_status, import_files_by_path,
    pick_and_import_files, pick_and_import_repair_asset, remove_media_library_asset,
    request_preview_proxy, request_waveform_analysis, run_media_diagnostics, start_media_recovery,
    AppInfo, MediaEngineState, MediaLibraryState, PlaybackState, RecoveryJobStatus,
    RepairDiagnosticsState, WaveformState,
};

/// Desktop application entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ai_state = commands::AiState::default();
    let transcription_state = commands::TranscriptionState::new(ai_state.manager.storage().clone());

    let review_base_dir = ai_state.manager.storage().base_dir().join("review");
    let review_storage =
        opentake_core::ai::translation::review::ReviewStorageManager::new(review_base_dir)
            .expect("Failed to initialize review storage");
    let translation_review_state = commands::TranslationReviewState::new(review_storage);

    let translation_state = commands::TranslationState::new(
        ai_state.manager.storage().clone(),
        Some(translation_review_state.storage.clone()),
    );
    let tts_state = commands::TtsState::new(ai_state.manager.storage().clone());
    let voice_library_state =
        commands::VoiceLibraryState::default_local(Arc::new(ai_state.manager.storage().clone()));
    let dubbing_state =
        commands::DubbingState::default_local(Arc::new(ai_state.manager.storage().clone()));

    tauri::Builder::default()
        .manage(commands::MediaEngineState::default())
        .manage(commands::MediaLibraryState::default())
        .manage(commands::PlaybackState::default())
        .manage(commands::WaveformState::default())
        .manage(commands::RepairDiagnosticsState::default())
        .manage(commands::StylingState::default())
        .manage(ai_state)
        .manage(transcription_state)
        .manage(translation_state)
        .manage(translation_review_state)
        .manage(tts_state)
        .manage(voice_library_state)
        .manage(dubbing_state)
        .register_uri_scheme_protocol("opentake-media", |app, req| {
            protocol::media::handle_media_protocol(app, req)
        })
        .invoke_handler(tauri::generate_handler![
            commands::ai::list_ai_models,
            commands::ai::get_ai_model_status,
            commands::ai::get_ai_hardware_info,
            commands::ai::start_ai_model_download,
            commands::ai::get_ai_model_download_status,
            commands::ai::cancel_ai_model_download,
            commands::ai::verify_ai_model,
            commands::ai::remove_ai_model,
            commands::ai::get_ai_storage_usage,
            commands::app_info::get_app_info,
            commands::media_engine::get_media_engine_status,
            commands::media_import::pick_and_import_files,
            commands::media_import::import_files_by_path,
            commands::media_import::get_media_library_assets,
            commands::media_import::remove_media_library_asset,
            commands::media_import::clear_media_library,
            commands::playback::get_media_playback_url,
            commands::playback::request_preview_proxy,
            commands::playback::cancel_preview_proxy,
            commands::playback::get_proxy_status,
            commands::waveform::get_waveform_status,
            commands::waveform::request_waveform_analysis,
            commands::waveform::cancel_waveform_analysis,
            commands::waveform::get_waveform_segment,
            commands::repair::run_media_diagnostics,
            commands::repair::cancel_media_diagnostics,
            commands::repair::get_diagnostic_report,
            commands::repair::export_diagnostic_report,
            commands::repair::pick_and_import_repair_asset,
            commands::repair::start_media_recovery,
            commands::repair::get_recovery_status,
            commands::repair::cancel_media_recovery,
            commands::repair::finalize_recovered_asset,
            commands::repair::export_recovery_report,
            commands::repair::pick_and_import_reference_asset,
            commands::repair::analyze_reference_compatibility,
            commands::repair::start_mp4_reconstruction,
            commands::repair::get_reconstruction_status,
            commands::repair::cancel_mp4_reconstruction,
            commands::repair::get_reconstruction_capabilities,
            commands::repair::export_reconstruction_report,
            commands::repair::register_recovery_candidate_preview,
            commands::repair::get_recovery_review,
            commands::repair::run_advanced_output_validation,
            commands::repair::export_recovery_review_report,
            commands::repair::get_recovery_history,
            commands::repair::clear_recovery_history,
            commands::subtitles::pick_and_import_subtitle_file,
            commands::subtitles::parse_subtitle_content,
            commands::subtitles::export_subtitle_track,
            commands::subtitles::save_subtitle_project,
            commands::subtitles::open_subtitle_project,
            commands::subtitles::validate_subtitle_document_command,
            commands::subtitles::apply_subtitle_operation_command,
            commands::transcription::get_whisper_runtime_capabilities,
            commands::transcription::select_whisper_runtime,
            commands::transcription::start_offline_transcription,
            commands::transcription::get_offline_transcription_status,
            commands::transcription::cancel_offline_transcription,
            commands::transcription::get_offline_transcription_result,
            commands::transcription::apply_transcription_result_to_subtitles,
            commands::alignment::get_word_alignment_result,
            commands::alignment::segment_words_to_cues,
            commands::alignment::refine_subtitle_cues,
            commands::alignment::update_word_timing_in_subtitle_cue,
            commands::alignment::apply_smart_segmented_subtitles,
            commands::styling::get_subtitle_style_presets,
            commands::styling::save_custom_subtitle_style_preset,
            commands::styling::delete_custom_subtitle_style_preset,
            commands::styling::get_system_fonts,
            commands::styling::generate_ass_script_preview,
            commands::styling::start_subtitle_burn_in_export,
            commands::styling::get_subtitle_burn_in_status,
            commands::styling::cancel_subtitle_burn_in_export,
            commands::styling::finalize_burn_in_export,
            commands::translation::get_translation_capabilities,
            commands::translation::list_translation_language_pairs,
            commands::translation::select_translation_runtime,
            commands::translation::start_offline_translation,
            commands::translation::get_offline_translation_status,
            commands::translation::cancel_offline_translation,
            commands::translation::get_offline_translation_result,
            commands::translation::apply_translation_result_to_subtitles,
            commands::translation_review::get_or_create_translation_review,
            commands::translation_review::save_translation_review_to_document,
            commands::translation_review::update_review_entry_target_text,
            commands::translation_review::set_review_entry_status_command,
            commands::translation_review::batch_set_review_status_command,
            commands::translation_review::list_glossaries_command,
            commands::translation_review::save_glossary_command,
            commands::translation_review::delete_glossary_command,
            commands::translation_review::get_translation_memory_command,
            commands::translation_review::record_approved_translation_command,
            commands::translation_review::lookup_translation_memory_command,
            commands::translation_review::delete_translation_memory_entry_command,
            commands::translation_review::clear_translation_memory_command,
            commands::tts::get_tts_runtime_capabilities,
            commands::tts::select_tts_runtime,
            commands::tts::list_available_tts_voices,
            commands::tts::get_tts_voice_status,
            commands::tts::start_offline_tts,
            commands::tts::get_offline_tts_status,
            commands::tts::cancel_offline_tts,
            commands::tts::get_offline_tts_result,
            commands::tts::export_tts_audio,
            commands::voices::list_voice_library,
            commands::voices::get_voice_details,
            commands::voices::get_voice_compatibility,
            commands::voices::start_voice_preview,
            commands::voices::compare_voices,
            commands::voices::get_voice_preferences,
            commands::voices::toggle_voice_favorite,
            commands::voices::set_preferred_voice,
            commands::voices::clear_voice_preview_cache,
            commands::voices::assign_voice_to_track_command,
            commands::voices::remove_voice_assignment_command,
            commands::voices::set_project_default_voice_command,
            commands::voices::get_project_voice_assignments_command,
            commands::dubbing::create_dubbing_project,
            commands::dubbing::get_active_dubbing_project,
            commands::dubbing::set_active_dubbing_project,
            commands::dubbing::load_dubbing_from_document,
            commands::dubbing::save_dubbing_to_document,
            commands::dubbing::save_dubbing_to_file,
            commands::dubbing::load_dubbing_from_file,
            commands::dubbing::generate_dubbing_segments,
            commands::dubbing::regenerate_dubbing_segment,
            commands::dubbing::apply_dubbing_operation,
            commands::dubbing::undo_dubbing_operation,
            commands::dubbing::redo_dubbing_operation,
            commands::dubbing::get_dubbing_waveform,
            commands::dubbing::get_dubbing_diagnostics,
            commands::dubbing::get_dubbing_asset_url,
            commands::dubbing::plan_dubbing_alignment,
            commands::dubbing::process_dubbing_alignment,
            commands::dubbing::apply_dubbing_alignment,
            commands::dubbing::restore_original_dubbing_audio,
            commands::dubbing::set_dubbing_alignment_mode,
            commands::dubbing::set_dubbing_start_offset,
            commands::dubbing::get_dubbing_alignment_diagnostics,
            commands::dubbing::batch_align_dubbing_segments,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenTake desktop application");
}
