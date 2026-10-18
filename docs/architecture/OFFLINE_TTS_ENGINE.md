# Offline AI Text-to-Speech Engine Architecture

## 1. Executive Summary

STEP 24 introduces a genuine, completely offline Text-to-Speech (TTS) synthesis engine into Open Media Workbench's **Localization Studio**. The engine enables users to synthesize high-quality speech from arbitrary text scripts or localized subtitle tracks using local CPU inference.

Key tenets:
- **Zero Cloud / Zero API Calls**: 100% offline local inference on CPU.
- **Privacy & Security**: User text scripts and subtitle dialogues are never transmitted over network or logged to disk. No shell execution.
- **Shared Model Registry**: Reuses the STEP 18 shared AI Model Manager, extended with backward-compatible multi-artifact verification for `.onnx` model weights and `.onnx.json` voice configurations.
- **Precise Timing & Non-destructive Dubbing Foundation**: Generates individual audio segments mapped to source subtitle cues, explicitly calculating timing differences (`timing_difference_us`) without silently truncating, stretching, or replacing original audio.

---

## 2. System Architecture

```
+-----------------------------------------------------------------------------------+
|                              Localization Studio                                  |
|         (TextToSpeechPanel: Script / Subtitle Source, Voice Picker, Controls)     |
+-----------------------------------------------------------------------------------+
                                          |
                                          | Tauri IPC Commands
                                          v
+-----------------------------------------------------------------------------------+
|                        Tauri Application Backend Bridge                           |
|       (src-tauri/src/commands/tts.rs: get_tts_status, start_offline_tts, etc.)     |
+-----------------------------------------------------------------------------------+
                                          |
                                          | Invokes
                                          v
+-----------------------------------------------------------------------------------+
|                                   TtsManager                                      |
|    - Concurrent job limits & lifecycle management                                 |
|    - AI Model storage locking (AiStorageManager)                                  |
|    - Text script sentence chunking / Subtitle cue extraction                      |
|    - Isolated temporary staging path generation                                   |
|    - WAV audio validation & concatenation                                         |
+-----------------------------------------------------------------------------------+
                        |                                       |
                        v                                       v
+----------------------------------+        +---------------------------------------+
|         TtsEngine Trait          |        |        AiStorageManager (STEP 18)     |
|  - MockTtsEngine (Unit tests)    |        |  - Model registry (~/.local/share/...) |
|  - SystemPiperCliEngine (Real)   |        |  - Multi-artifact verification        |
+----------------------------------+        +---------------------------------------+
                |
                v Process execution
+-----------------------------------------------------------------------------------+
|                Piper Offline TTS Runtime (v2023.11.14-2 / 1.2.0)                  |
|  - Binary: ~/.local/share/opentake/bin/piper                                      |
|  - Libraries: libespeak-ng, libonnxruntime, libpiper_phonemize                    |
|  - Voice: piper-en-lessac-medium (en_US-lessac-medium.onnx + .onnx.json)          |
+-----------------------------------------------------------------------------------+
                                          |
                                          v Writes validated PCM 16-bit WAV
+-----------------------------------------------------------------------------------+
|                                Staged WAV Audio                                   |
|  - Secure staging directory: ~/.local/share/opentake/staging/tts/                 |
|  - Sample rate: 22050 Hz, Channels: 1 (Mono), Format: PCM 16-bit signed           |
|  - Validated by validate_wav_file (RIFF/WAVE header, non-silent RMS check)        |
+-----------------------------------------------------------------------------------+
                        |                                       |
                        v                                       v
         +-----------------------------+        +-------------------------------+
         |     HTML5 Audio Preview     |        |       Atomic WAV Export       |
         | (Segment & Combined Player) |        | (Safe destination dialog,     |
         +-----------------------------+        |  no-clobber write)            |
                                                +-------------------------------+
```

---

## 3. Runtime Selection & Supply Chain

### Primary Runtime: Piper TTS
- **Upstream**: [rhasspy/piper](https://github.com/rhasspy/piper)
- **Version**: `2023.11.14-2` (binary reporting `1.2.0`)
- **License**: MIT License
- **Dependencies Bundled**:
  - `libespeak-ng.so.1.52.0.1` (GPL-3.0-or-later)
  - `libonnxruntime.so.1.14.1` (MIT License)
  - `libpiper_phonemize.so.1.2.0` (MIT License)
  - `espeak-ng-data/` phoneme dictionary
- **Execution Strategy**:
  - Directly invoked via Rust `std::process::Command` without shell execution.
  - Runtime binary located in `~/.local/share/opentake/bin/piper` or user-configured executable.
  - `LD_LIBRARY_PATH` dynamically configured to the directory containing the runtime binary so bundled ONNX Runtime and espeak-ng libraries are loaded securely without polluting global system paths.
  - Script text is piped securely via `stdin` to prevent command-line argument length limits and prevent exposure in process listing tables (`ps`).
  - Process group management enables clean, immediate cancellation on user request.

### Primary Voice Model: `piper-en-lessac-medium`
- **Voice Character**: English (United States), Lessac dataset, medium quality.
- **License**: Public Domain (The Lessac voice dataset was recorded and released into the public domain by Donald Derrick).
- **Artifacts**:
  1. `en_US-lessac-medium.onnx` (63,201,294 bytes, SHA-256: `5efe09e69902187827af646e1a6e9d269dee769f9877d17b16b1b46eeaaf019f`)
  2. `en_US-lessac-medium.onnx.json` (4,885 bytes, SHA-256: `efe19c417bed055f2d69908248c6ba650fa135bc868b0e6abb3da181dab690a0`)
- **Audio Output**: 22,050 Hz, 1-channel Mono, 16-bit PCM.

---

## 4. Multi-Artifact Model Manifest Schema

To support Piper-style voices requiring both model weights and JSON configuration without breaking existing single-file Whisper GGML and Qwen GGUF models, the AI Model schema was extended:

```rust
pub struct AiModelManifest {
    pub id: String,
    pub name: String,
    pub task: AiTask, // Added AiTask::TextToSpeech
    pub artifact_filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub additional_artifacts: Vec<AiModelArtifact>, // New field
    // ...
}

pub struct AiModelArtifact {
    pub filename: String,
    pub role: String, // e.g., "config", "phoneme_table"
    pub size_bytes: u64,
    pub sha256: String,
    pub download_url: Option<String>,
}
```

The storage manager verifies all artifacts during installation:
- If any required artifact is missing or fails its cryptographic hash verification, the voice model is rejected and cannot be used for inference.

---

## 5. Subtitle-to-Speech Mapping & Timing Contracts

When synthesizing from a `SubtitleTrack`:
1. Each subtitle cue is synthesized individually into a `TtsAudioSegment`.
2. The segment records:
   - `cue_id`: Source subtitle cue identifier.
   - `cue_start_us`: Subtitle display start timestamp in microseconds.
   - `cue_end_us`: Subtitle display end timestamp in microseconds.
   - `cue_duration_us`: Duration of the subtitle display interval (`cue_end_us - cue_start_us`).
   - `duration_us`: Measured duration of the generated speech audio from PCM sample count.
   - `timing_difference_us`: Signed timing delta (`(duration_us as i64) - (cue_duration_us as i64)`).
3. **No Silent Stretching / No Truncation**:
   - The engine **never** time-stretches speech or truncates speech to forcibly fit cue intervals.
   - Any difference between subtitle duration and speech duration is reported explicitly in the UI, and fed directly into the Duration Alignment engine (STEP 27) for pitch-preserving time-stretching or silence padding.
   - Original subtitle tracks remain completely immutable.

---

## 6. Audio Validation & Export

- **WAV Validation (`validate_wav_file`)**:
  - Parses RIFF and WAVE chunk headers.
  - Validates `fmt ` chunk: PCM audio format (tag 1), valid channel count (1 or 2), valid sample rate (8000–48000 Hz).
  - Validates `data` chunk: non-empty sample buffer.
  - Samples analyzed for acoustic energy: RMS amplitude, peak sample amplitude, and silence detection. Pure silence files or truncated headers are rejected.
- **WAV Concatenation (`concatenate_wav_files`)**:
  - Allows exporting multi-segment script synthesis as a single unified WAV file.
  - Verifies that all segment files share identical sample rates, bit depths, and channel layouts before byte-level PCM streaming concatenation.
- **Safe Export (`export_tts_audio`)**:
  - Validates output file destination.
  - Verifies that destination is not inside source media directories.
  - Atomic write with no-clobber protection unless explicitly authorized.

---

## 7. Security Boundaries & Privacy

1. **No Cloud Calls**: Offline operation is strictly enforced.
2. **Model Locking**: Inference locks the voice model in `AiStorageManager`, preventing deletion or replacement while active jobs are running.
3. **Staging Isolation**: Temporary WAV files reside strictly in `~/.local/share/opentake/staging/tts/<job_id>/`.
4. **Cancellation**: Process groups are cleanly terminated, resources unlocked, and partial staging cleaned up automatically.
5. **No Telemetry**: Zero user data collection.

---

## 8. Integration with Offline AI Voice Library (STEP 25)

The TTS engine provides the underlying speech synthesis foundation for the STEP 25 AI Voice Library (`crates/opentake-core/src/ai/voices/`):
- **Voice Previews**: `VoicePreviewManager` invokes `TtsEngine::synthesize_speech()` to produce genuine offline speech previews for catalog voices.
- **WAV Validation**: Uses `validate_wav_file` to inspect generated preview audio.
- **Canonical Voice Identity**: Both `TextToSpeechPanel` and `VoiceLibraryPanel` resolve voices against the canonical catalog, honoring project track assignments and user preferences.
- **Shared Model Locking**: Previews acquire model locks in `AiStorageManager` during synthesis to prevent concurrent file mutations.

---

## 9. Integration with Professional AI Dubbing Timeline (STEP 26)

The TTS engine drives multi-track speech synthesis and segment regeneration in the STEP 26 Dubbing Timeline (`crates/opentake-core/src/localization/dubbing/`):
- **Batch Cue Synthesis**: `DubbingManager::synthesize_and_place_cues` routes subtitle cue text through `TtsManager`, generating genuine 16-bit PCM WAV audio for each selected cue.
- **Durable Asset Promotion**: Staged TTS audio files are atomically imported into durable dubbing storage (`~/.local/share/opentake/dubbing/assets`), decoupling saved projects from temporary staging directories.
- **Segment Regeneration with Rollback**: `DubbingManager::regenerate_segment` executes targeted cue re-synthesis while safeguarding the previous working audio asset in case synthesis fails or is cancelled.
- **Exact Sample Duration**: Timeline placement computes `measured_duration_us` directly from the generated WAV samples, reliably detecting overrun conditions (`cue_overrun_us`) against subtitle intervals without destructive time-stretching.

