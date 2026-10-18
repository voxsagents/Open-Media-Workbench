# Offline AI Voice Library Architecture

## 1. Executive Summary

STEP 25 introduces a professional, offline-first **AI Voice Library** for Open Media Workbench's **Localization Studio**. The Voice Library provides a unified domain for voice discovery, genuine offline voice previews, A/B voice comparisons, standards-based language and locale filtering, favorites and default preferences, multi-speaker selection, and project track-level voice assignments.

Key tenets:
- **100% Offline Local Inference**: All speech synthesis utilizes the verified offline Piper runtime on CPU. Zero cloud API calls, zero telemetry, zero accounts.
- **Genuine Offline Previews**: No prerecorded generic audio or fabricated waveforms. Voice previews synthesize actual audio via the core TTS engine.
- **Bounded LRU Preview Cache**: Content-addressed cache keys (`voice_id:model_hash:speaker:settings:text_hash`) prevent redundant synthesis, with automated LRU eviction (50 entries, 50MB disk limit) and cache invalidation upon model revisions.
- **Shared Model Architecture**: Extends and integrates directly with the existing STEP 18 AI Model Manager and STEP 24 TTS engine without creating duplicate registries or alternative model storage paths.
- **Track & Project Level Assignments**: Allows users to bind voices and speaker variants to subtitle tracks or set project defaults via versioned metadata (`SubtitleDocument.metadata.voiceAssignments`), fully preserving backwards compatibility and maintaining subtitle cue timing without alteration.
- **A/B Comparison with Honest Reporting**: Side-by-side comparison between two distinct, installed, and ready voices using identical sample text. If only one voice is ready, the interface honestly indicates that another voice must be installed rather than fabricating a comparison.

---

## 2. System Architecture

```
+-----------------------------------------------------------------------------------+
|                              Localization Studio                                  |
|     (VoiceLibraryPanel: Catalog, Search, Filters, Real Preview, A/B Comparison)   |
+-----------------------------------------------------------------------------------+
                                          |
                                          | Tauri IPC Commands
                                          v
+-----------------------------------------------------------------------------------+
|                        Tauri Application Backend Bridge                           |
|       (src-tauri/src/commands/voices.rs: list_voice_library, preview, etc.)       |
+-----------------------------------------------------------------------------------+
                                          |
                                          | Invokes
                                          v
+-----------------------------------------------------------------------------------+
|                              VoiceLibraryManager                                  |
|   - Catalog discovery & standards-based language/locale queries                  |
|   - Multi-stage server-side compatibility checks (AiStorageManager + TtsEngine)   |
|   - Atomic voice preferences persistence (favorites, recents, default voice)      |
|   - Bounded LRU preview cache manager with content-addressed keys                 |
|   - Real preview synthesis & side-by-side voice comparison engine                 |
|   - SubtitleDocument track & project default voice assignment contracts          |
+-----------------------------------------------------------------------------------+
        |                            |                           |
        v                            v                           v
+------------------+       +-------------------+       +--------------------+
|  TtsEngine (24)  |       | AiStorageManager  |       |  SubtitleDocument  |
|  - Piper CLI     |       | (STEP 18 models)  |       |  (STEP 16-24 doc)  |
|  - Real WAV gen  |       | - SHA-256 verify  |       |  - Voice metadata  |
+------------------+       +-------------------+       +--------------------+
```

---

## 3. Rust Domain Modules

The domain logic resides in `crates/opentake-core/src/ai/voices/`:

| Module | Responsibility |
| :--- | :--- |
| `catalog.rs` | Curated catalog of verified voice models, language extraction, locale filtering, and metadata lookups. |
| `models.rs` | Strongly-typed Rust data structures (`VoiceCatalogEntry`, `VoiceAvailabilityStatus`, `VoiceCompatibility`, `VoicePreferences`, `VoiceAssignment`, `ProjectVoiceAssignments`, `VoicePreviewRequest`, `VoicePreviewResult`, `VoiceComparisonRequest`, `VoiceComparisonResult`). |
| `compatibility.rs` | Comprehensive server-side readiness verification inspecting catalog entry, installation state, artifact completeness (`.onnx` + `.onnx.json`), SHA-256 integrity, runtime presence, language support, and speaker ID boundaries. |
| `preferences.rs` | `VoicePreferencesManager`: atomic JSON persistence (`voice_preferences.json`) of favorites, recent voices, global preferred default voice, and last selected speaker variant. |
| `previews.rs` | `VoicePreviewManager`: content-addressed preview synthesis, bounded cache eviction (50 files, 50MB limit), model lock acquisition, WAV validation, and A/B voice comparison. |
| `assignments.rs` | Serialization and management of project default voice and track voice assignments stored cleanly within `SubtitleDocument.metadata["voiceAssignments"]`. |
| `errors.rs` | `VoiceLibraryError` enum covering unknown voices, missing models, runtime errors, invalid speakers, and I/O failures. |
| `mod.rs` | `VoiceLibraryManager` facade uniting all sub-modules for clean integration. |

---

## 4. Verified Catalog Voices

| Voice ID | Display Name | Language | Locale | Quality | Speakers | License | Commercial |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `piper-en-lessac-medium` | Lessac (English US) | `en` | `en-US` | Medium | 1 | Open Software License | Permissive |
| `piper-en-libritts-high` | LibriTTS Multi-Speaker | `en` | `en-US` | High | 904 | CC-BY-4.0 | Permissive with attribution |
| `piper-es-sharvard-medium` | Sharvard (Spanish ES) | `es` | `es-ES` | Medium | 1 | Open Software License | Permissive |
| `piper-bn-ibrahim-medium` | Ibrahim (Bengali BD) | `bn` | `bn-BD` | Medium | 1 | Open Software License | Permissive |

---

## 5. Compatibility Verification Engine

The compatibility engine evaluates models across multiple stages:
1. **Catalog Verification**: Must belong to the verified AI voice catalog.
2. **Model Installation**: Checked via `AiStorageManager::get_installation()`.
3. **Artifact Completeness**: Both `.onnx` and `.onnx.json` files must be present and verified against SHA-256 hashes.
4. **Runtime Readiness**: Verified via `TtsEngine::verify_runtime()`.
5. **Language Match**: If a target language is requested, verified against supported languages without cross-lingual hallucination.
6. **Speaker Boundaries**: If a speaker ID is specified, verified against `supported_speaker_ids` range.

Statuses reported:
- `Ready`: Model is installed, verified, runtime is active, and settings are compatible.
- `AvailableForInstall`: Verified model in catalog, but weights/configs not yet downloaded locally.
- `InstalledRuntimeMissing`: Model is downloaded, but Piper executable is not installed or configured.
- `InstalledVerificationRequired`: Model files are present but require SHA-256 verification.
- `UnsupportedLanguage`: Voice does not support the requested target language.
- `Incompatible`: Required architecture or settings not supported by current runtime.
- `Corrupted`: Artifact SHA-256 or size mismatch on disk.

---

## 6. Secure Bounded Preview Cache & Privacy

Preview audio is cached using content-addressed SHA-256 keys derived from:
`voice_id : model_revision : speaker_id : settings : sha256(normalized_text)`

- **Storage Location**: `~/.local/share/opentake/cache/voice_previews/`
- **Cache Size Boundaries**: Maximum 50 preview files or 50 MB total disk usage.
- **Eviction Policy**: Least Recently Used (LRU) based on file modification timestamps.
- **Atomic Operations**: Synthesis outputs to temporary `.part.wav` before atomic rename.
- **Privacy & Security**: Preview text is hashed; raw text is never used in filenames or logged to disk. Users can clear the cache at any time using the "Clear Cache" button or IPC command.

---

## 7. Project Voice Assignment Contract

Voice assignments are stored non-destructively in `SubtitleDocument.metadata["voiceAssignments"]`:

```json
{
  "defaultVoiceId": "piper-en-lessac-medium",
  "trackAssignments": [
    {
      "trackId": "tr_en_dialogue",
      "voiceId": "piper-en-lessac-medium",
      "language": "en",
      "speakerId": 0,
      "assignedAtEpochMs": 1729000000000
    }
  ]
}
```

Voice assignments:
- Do not alter subtitle cue text.
- Do not alter subtitle cue start/end timings.
- Do not synthesize speech automatically upon assignment.
- Provide contracts for STEP 26 timeline construction.

---

## 8. Dubbing Timeline Integration (STEP 26)

The Dubbing Timeline in STEP 26 consumes track voice assignments directly from `SubtitleDocument.metadata["voiceAssignments"]`:
- **Timeline Track Initialization**: When creating dubbing tracks from subtitle tracks, `DubbingManager` reads the assigned `voice_id` and `speaker_id` to establish speech synthesis configuration.
- **Compatibility Verification**: Voice assignments are validated against installed catalog voices before triggering batch synthesis or per-segment regeneration. Uninstalled voices surface `UNRESOLVED_VOICE_ASSIGNMENT` diagnostics.
- **Per-Track & Segment Configuration**: Users can reassign voices on timeline tracks or individual segments directly within `DubbingTimelinePanel`, seamlessly syncing changes back into project state.
