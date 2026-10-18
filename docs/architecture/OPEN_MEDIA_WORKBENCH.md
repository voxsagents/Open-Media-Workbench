# Open Media Workbench — Unified Desktop Architecture

**Product Name:** Open Media Workbench  
**Document Version:** 1.0.0  
**Target Release:** v0.1.0 Beta  
**Baseline Commit:** `663331b8c5fc0dcb5326a1bf8517125afb9ab8a1`  
**Date:** 2026-10-14  

---

## 1. Vision & Architecture Paradigm

Open Media Workbench is a high-performance, offline-first desktop application engineered as:
> **ONE native desktop application hosting TWO specialized, independent studios:**
> 1. **Repair Studio:** Video corruption diagnosis, MP4/MOV recovery, elementary stream salvage, and repair validation.
> 2. **Localization Studio:** Subtitle editing, local AI speech-to-text, translation, neural speech dubbing, and export.

Rather than fragmenting into multiple disparate tools, both studios operate on top of a single, battle-tested, headless **Rust Media Core** (`opentake-core`) wrapped in a security-hardened **Tauri 2** desktop container with a reactive **React 19 + TypeScript** user interface.

```
┌────────────────────────────────────────────────────────────────────────┐
│               Open Media Workbench Shell (React 19)                    │
│  ┌──────────────────────────────┐    ┌──────────────────────────────┐  │
│  │     🛠️ Repair Studio         │    │   🌐 Localization Studio     │  │
│  │ (Diagnosis, Salvage, Fixes)  │    │ (Subs, ASR, Dubbing, Export) │  │
│  └──────────────┬───────────────┘    └──────────────┬───────────────┘  │
│                 │                                   │                  │
│                 └─────────────────┬─────────────────┘                  │
│                                   │                                    │
│       Shared UI Services: VideoPreview, WaveformCanvas, Timeline       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Tauri 2 IPC & Custom URI Protocol
┌───────────────────────────────────┴────────────────────────────────────┐
│                    Tauri 2 Native Desktop Layer                        │
│  ├── Window Management & Menus                                         │
│  ├── Custom Streaming Protocol (opentake-media://)                     │
│  ├── File Authorization & Prov-Tracking                                │
│  └── Background Task Cancellation & Event Dispatch                     │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Native Crate Dependency
┌───────────────────────────────────┴────────────────────────────────────┐
│                    Rust Media Core (opentake-core)                     │
│  ├── media::process      (Managed, secure FFmpeg/FFprobe sub-processes)│
│  ├── media::range        (Streaming byte-range HTTP protocol provider) │
│  ├── media::waveform     (Multi-LOD audio peak/RMS analysis & cache)   │
│  ├── media::proxy        (Low-latency 720p H.264 proxy generator)      │
│  └── editing::*          (Canonical integer microsecond EDL engine)    │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Shared Rust Media Core & Completed Infrastructure (Steps 1–9)

The foundational work completed across STEPS 1 through 9 remains completely preserved and serves as the common substrate for both studios:

1. **Process Management Infrastructure (`media::process`)**:
   - Secure execution of FFmpeg and FFprobe without shell interpolation.
   - POSIX process group isolation (`setpgid`) allowing deterministic, leak-free termination of runaway transcoding jobs.
   - Memory-bounded stderr parsing and output scrubbing.
2. **Media Inspection & Metadata Probing (`media::metadata`)**:
   - Extraction of video/audio container attributes using rational arithmetic (`Rational { num, den }`).
   - Strict timebase conversions avoiding floating-point inaccuracies.
3. **Secure Custom URI Media Transport (`media::range`)**:
   - Custom desktop protocol handler registered with WebKitGTK and webview.
   - High-performance HTTP 206 Partial Content byte-range serving directly from disk with sub-millisecond seek response.
4. **Proxy Transcoding Pipeline (`media::proxy`)**:
   - Content-addressed Blake3 cache keys for high-bitrate or non-browser-native codecs (HEVC, ProRes, AV1).
   - Fast-start MP4 (`+faststart`) generation with local cache invalidation.
5. **Multi-LOD Audio Waveform Extraction (`media::waveform`)**:
   - Background audio decimation to 16 kHz mono/stereo PCM.
   - Multi-resolution hierarchical peak and RMS cache files with fast memory-mapped chunk loading.
6. **Canonical Non-Destructive EDL Model (`editing::*`)**:
   - Integer microsecond representation (`MediaTimeUs`, `SourceRange`).
   - Immutable timeline manipulation primitives (`cut_source_range`, `restore_source_range`, `keep_only`).
   - $O(\log N)$ coordinate transformation index (`TimelineIndex`) between source time and timeline playhead.

---

## 3. Studio Boundaries & Functional Allocation

### 3.1 Repair Studio (Workspace 1)
**Objective:** Ingest broken, incomplete, or corrupted video files and recover playable media streams without destructive writes.

- **Ingestion & Diagnostics Boundary (Completed in STEP 12):**
  - **Read-Only Forensic Inspection:** Zero mutation of original media files; strict verification of source provenance and media library authorization.
  - **Signature & Container Identification (`repair::container`):** Magic byte detection for MP4/MOV, MKV EBML, and AVI RIFF with extension mismatch detection.
  - **Bounded ISO BMFF Atom Scanner (`repair::iso_bmff`):** Memory-bounded scan of top-level boxes (`ftyp`, `moov`, `mdat`, `free/skip`, `moof`, `sidx`). Safely handles 64-bit extended box sizes, size-zero boxes extending to EOF, and truncated headers with checked arithmetic.
  - **Stream Track Inspection (`repair::streams`):** Probes stream dimensions, timebases, and VFR detection via managed FFprobe, converting probe errors (like `moov atom not found`) into structured diagnostic findings without crashing.
  - **Bounded Decode & Timestamp Verification (`repair::timestamps`):** Null-sink test decode (`-f null -`) scanning up to 30s for non-monotonic DTS jumps, macroblock concealment, and corrupted NAL units.
  - **Evidence-Based Recommendations (`repair::report`):** Suggests loss-less remuxing (STEP 13), reference-based MOOV reconstruction (STEP 14), or stream salvage (STEP 13) with explicit target steps and disabled execution.
  - **Sanitized Report Export:** Native user-directed export of versioned JSON diagnostic reports with sanitized paths.
- **Recovery Pipeline & Salvage Engine (Completed in STEP 13):**
  - **Lossless Container Remux (`repair::remux`):** Pure stream copy (`-c copy`) of readable streams into clean, compliant containers (MP4, MKV, MOV, AVI) with faststart indexing, without lossy re-encoding.
  - **Conservative Stream Salvage (`repair::salvage`):** Isolate intact video or audio tracks when sister streams suffer from severe defects or demux failures (Video-Only, Audio-Only, All Readable).
  - **Timestamp Continuity Rebuild (`repair::remux`):** Controlled timestamp regeneration (`+genpts`, `avoid_negative_ts make_zero`) resolving non-monotonic DTS/PTS jumps and audio/video sync drift.
  - **Bounded Output Validation (`repair::validation`):** Three-stage output verification (file size, container recognition, stream loss check, and bounded decode test).
  - **Secure Application Staging & No-Clobber Finalization (`repair::recovery`):** Isolated staging directory execution, strict no-overwrite protection, and symlink attack prevention.
  - **Asset Integration & Privacy-Preserving Reporting (`repair::recovery_report`):** Registers validated output into the active media library as a new authorized asset for instant preview, with sanitized JSON report export.
- **Reference-Based Reconstruction (Completed in STEP 14):**
  - Reference-based MOOV atom reconstruction (`repair::reconstruction`, `repair::reference`): extracts track and codec configuration (SPS/PPS parameter sets, timescale) from a healthy donor file recorded on the same device to synthesize missing sample tables (`stbl`) in unfinalized or interrupted recordings.
  - Dual authorization verification through `MediaAuthorizationRegistry`, built-in safe Rust reconstructor with pluggable external `untrunc` adapter, bounded decode validation, and zero-byte source/reference immutability.
- **Recovery Review, Preview & Reporting (Completed in STEP 15):**
  - Unified `RecoveryReview` domain model synthesizing diagnostic, recovery, and reconstruction evidence.
  - Safe candidate preview via `SourceProvenance::StagedRecoveryCandidate` and opaque streaming tokens.
  - Multi-mode player with side-by-side (`Original | Recovered`), recovered-only, and original-only playback with honest unplayable-original fallback.
  - Multi-tier validation inspector (`MetadataOnly`, `SampledDecode`, and on-demand `FullDecode`).
  - Timeline difference analysis without fabricated percentages or invented durations.
  - Crash-safe, atomic versioned JSON history ledger (up to 50 attempts).
  - Standalone offline HTML and machine-readable JSON report export with home directory and staging path privacy sanitization.

### 3.2 Localization Studio (Active Development — STEPS 16–25 Complete)
**Objective:** End-to-end subtitle generation, interactive visual editing, speech recognition, machine translation, voice discovery, and neural speech dubbing.

- **Canonical Subtitle Domain Model & Timing (Completed in STEP 16):**
  - **Microsecond Precision:** Cues and tracks represent time exclusively via `MediaTimeUs` ($t \ge 0$), completely eliminating floating-point rounding drifts.
  - **Dual Standards Ingestion:** Strict-but-resilient SRT parsing (handling arbitrary line breaks, BOMs, non-sequential indexes) and standards-aware WebVTT parsing (with `NOTE`, cue settings, and voice markup extraction).
  - **Structured Validation Engine:** Automatic detection of empty tracks, inverted intervals, duplicate IDs, out-of-bounds cues, and non-fatal overlapping cues with stable finding codes.
  - **Native Desktop Security & Provenance:** Subtitle file import enforces `SourceProvenance::SubtitleFileDialog` via `rfd`, 10 MB bounded file size limits, 20,000 max cue limits, and explicit binding to authorized media assets without media file mutation.
  - **Safe Atomic Export & Versioned Persistence:** Native export to compliant `.srt` and `.vtt` formats with atomic write/rename guarantees; versioned `.omwsub` project document format for multi-track persistence and reopening.
  - **Live Preview Caption Overlay & Time Sync:** Synchronized plain-text subtitle rendering in video preview monitor with CC visibility toggle and click-to-seek cue list navigation ($O(\log N)$ binary search lookup).
- **Professional Subtitle Timeline Editor (Completed in STEP 17):**
  - **Visual Cue Blocks:** Timeline cue blocks aligned with the shared time ruler and audio waveform canvas on a dedicated `S1 Subtitle` lane.
  - **Direct Manipulation:** Body dragging for time translation; left and right edge boundary handles for start and end resizing.
  - **Deterministic Snapping Engine:** Snaps to current playhead, neighboring cue start/end boundaries, and timeline grid with visual amber guides and configurable threshold.
  - **Split & Merge:** Split cues at playhead with clean Unicode/Bengali word and newline division; merge adjacent cues with deterministic text concatenation.
  - **Precise Timing Inspector:** `HH:MM:SS.mmm` controls, duration display, nudge buttons (-100ms, +100ms, -500ms, +500ms), and playhead sync buttons ("Start @ Playhead", "End @ Playhead").
  - **Dialogue Text Editor:** Multiline text editor with live character counts, reading speed (chars/sec), and non-blocking readability warnings (>42 chars/line).
  - **Multi-Cue Batch Editing:** Ctrl/Cmd-click and Shift-click range selection, batch nudge timing shifts with atomic negative-time rejection, and batch deletion.
  - **Multi-Level Undo/Redo:** 50-step deterministic history stack with `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`, and `Ctrl/Cmd+Y` shortcuts, designed to never intercept native text input undo events.
  - **Autosave & Crash Recovery:** Dirty state tracking with debounced snapshots and automatic recovery prompts on launch protecting against abrupt crashes or power loss.
- **Offline Whisper Speech-to-Text (Completed in STEP 19):**
  - **Local Whisper Inference:** High-efficiency `whisper.cpp` runtime adapter using local CPU threads without cloud telemetry or external network calls.
  - **Automated Subtitle Track Generation:** Media audio extraction to 16 kHz mono signed 16-bit PCM WAV via managed FFmpeg process and automatic canonical `SubtitleTrack` construction.
  - **Model Verification & Locking:** Concurrency-protected model access via the shared AI Model Manager (`~/.local/share/opentake/models`), with SHA-256 integrity checks.
  - **Multilingual Support:** Auto-detection and 90+ supported languages on multilingual models; strict rejection of non-English audio on English-only models.
- **Word-Level Timing Alignment & Smart Segmentation (Completed in STEP 20):**
  - **Word-Level Timestamp Extraction:** Genuine word and token boundary extraction from `whisper-cli` output (`tokens` array with microsecond precision), rejecting fabricated or evenly spaced timings.
  - **Typed Alignment Domain:** Full provenance tracking (`WhisperNativeEstimate`, `ForcedAlignment`, `UserAdjusted`, `Unaligned`) and timing quality scoring (`Aligned`, `Estimated`, `PartiallyAligned`, `Unavailable`).
  - **Smart Rule-Based Segmentation:** Configurable line wrapping, max chars per line, max duration, reading speed (CPS), silence gap detection, and sentence punctuation awareness preserving full transcript fidelity.
  - **Unicode-Safe Bengali Segmentation:** Strict grapheme-cluster preservation preventing combining mark splitting, nukta or virama truncation, and Bengali punctuation handling (Dari `।`).
  - **Karaoke Word Highlighting:** Zero-cost DOM word highlighting in `SubtitleOverlay` during playback without `dangerouslySetInnerHTML`.
  - **Waveform Timeline Word Markers:** Visual boundary ticks displayed on cue blocks at high zoom levels ($W_{\text{cue}} \ge 5\%$).
- **Professional Subtitle Styling & Video Burn-In Export (Completed in STEP 21):**
  - **Comprehensive Subtitle Styling:** Canonical style domain covering font family, size, weight, outline, shadow, background box, and word-level highlight color.
  - **8 Built-in Presets:** Clean Minimal, Cinematic, Modern Outline, Social Bold, Karaoke Highlight, Documentary, Accessible High Contrast, and Soft Background Box, plus custom presets.
  - **ASS Script Generation & FFmpeg Burn-In:** Safe SubStation Alpha generation with accurate timing, word karaoke tags (`\kf`), and production video re-encoding via FFmpeg with libass.
- **Offline Multilingual Subtitle Translation Engine (Completed in STEP 22):**
  - **Local CPU Inference:** Real on-device neural translation using `llama.cpp` CLI executing curated compact GGUF models (`Qwen 2.5 0.5B Instruct`). Zero cloud calls or telemetry.
  - **Authoritative Language Pair Capabilities:** Verified pair: English to Spanish (`en` -> `es`); experimental pairs for Bengali (`en` -> `bn`, `bn` -> `en`).
  - **Strict Timing Preservation:** Every translated cue maintains exact microsecond timestamps (`target_start_us == source_start_us`, `target_end_us == source_end_us`).
  - **Non-Destructive Workflow:** Original subtitle tracks are immutable; translations create a NEW `SubtitleTrack` in `.omwsub` with cleared word timings.
- **Bilingual Subtitle Review & Terminology Management (Completed in STEP 23):**
  - **Side-by-Side Bilingual Review:** Dual-track synchronization rendering original read-only source text alongside editable translated target cues with synchronized video playhead navigation.
  - **Stable Cue Pairing & Invalidation:** Cues matched by immutable `source_cue_id` and timing rather than naive array indices. Material text or timing edits immediately invalidate approved statuses (`Approved` -> `NeedsReview`), preventing stale approvals.
  - **Terminology Protection & Glossaries:** Local glossaries with exact, case-sensitive/insensitive, and whole-word matching. Protected brand names (e.g., Open Media Workbench, Whisper, FFmpeg) masked during translation and verified during review.
  - **Translation Memory (TM):** Local, privacy-preserving translation memory with exact-match suggestions, one-click application, and complete user deletion controls.
  - **Batch Operations & Quality Diagnostics:** Rule-based quality findings (`EMPTY_TARGET_TEXT`, `PROTECTED_TERM_CHANGED`, `EXCESSIVE_READING_SPEED`, `LANGUAGE_PAIR_UNVERIFIED`), batch approvals, and review progress statistics.
- **Offline AI Text-to-Speech Engine (Completed in STEP 24):**
  - **Local Speech Synthesis:** Genuine on-device speech synthesis using the official Piper TTS runtime (`v2023.11.14-2`, MIT) executing verified ONNX voice models (`piper-en-lessac-medium`, 22,050 Hz 16-bit mono PCM). 100% offline, zero cloud calls, no telemetry, no mandatory accounts.
  - **Script & Subtitle Sources:** Direct plain-text script synthesis with grapheme/Unicode-safe sentence chunking, or subtitle track synthesis generating per-cue speech segments.
  - **Timing Discrepancy Reporting:** Explicitly calculates `timing_difference_us` between measured audio duration and subtitle cue display duration. Never stretches or truncates speech silently; original subtitle tracks remain immutable.
  - **Audio Preview & WAV Export:** HTML5 audio playback preview of segments or concatenated sequences; safe atomic WAV file export with format and acoustic non-silence verification.
- **Professional Offline AI Voice Library (Completed in STEP 25):**
  - **Curated Offline Voice Catalog:** Verified offline voice models (`piper-en-lessac-medium`, `piper-en-libritts-high`, `piper-es-sharvard-medium`, `piper-bn-ibrahim-medium`) with display names, model IDs, quality tiers, sizes, and explicit licensing details.
  - **Search & Standards-Based Filtering:** Search voices by name or model; filter by ISO 639-1 language, BCP 47 locale, and installation status. Languages only appear selectable when a verified voice exists.
  - **Server-Side Compatibility Engine:** Multi-stage server-side validation evaluating catalog membership, installation state, artifact completeness (`.onnx` + `.onnx.json`), SHA-256 integrity, runtime presence, language support, and speaker ID boundaries.
  - **Genuine Offline Voice Preview:** Synthesizes authentic 16-bit PCM WAV audio using local Piper inference on CPU without generic sample substitution, fabricated waveforms, or cloud APIs. Features HTML5 audio playback with pause, seek, and accurate audio duration reporting.
  - **Side-by-Side Voice A/B Comparison:** Side-by-side comparison between two distinct, installed, and ready voices on identical sample text. If only one voice is ready, honestly informs user to install another voice via Model Manager.
  - **Bounded LRU Preview Cache:** Content-addressed preview cache (`voice_id:model_hash:speaker:settings:text_hash`) bounded to 50 files and 50 MB with automatic LRU eviction, model invalidation, and explicit clear cache action.
  - **Voice Preferences & Favorites:** Persistent favorites, recents, global preferred default voice, and last selected speaker variant saved atomically in `voice_preferences.json`.
  - **Track-Level Voice & Speaker Assignments:** Non-destructively binds voices and multi-speaker variants to subtitle tracks and establishes project defaults in `SubtitleDocument.metadata.voiceAssignments` without altering subtitle text or timestamps.
  - **Text-to-Speech Integration:** Seamless canonical voice synchronization between Voice Library, `LocalizationInspector`, and `TextToSpeechPanel`.
- **Professional AI Dubbing Timeline (STEP 26 Implemented):**
  - **Multi-Track Non-Destructive Audio Editor:** Dedicated Dubbing Timeline workspace in Localization Studio displaying reference video, original audio, source subtitles, translated subtitles, and AI speech tracks.
  - **Microsecond Timeline Invariants:** Integer microsecond coordinates (`MediaTimeUs`), non-destructive trim boundaries (`source_in_us`, `source_out_us`), and exact duration arithmetic.
  - **Real PCM Waveform Extraction:** Authentically extracts min/max peak bins directly from 16-bit PCM WAV assets without decorative fabrication or render decoding overhead.
  - **Non-Destructive Operations:** Move, trim start, trim end, split at playhead into adjacent non-overlapping segments, duplicate, and delete without rewriting underlying audio files.
  - **Durable Audio Asset Storage:** Manages generated WAV assets in `~/.local/share/opentake/dubbing/assets` with content hashing, surviving application restarts.
  - **Secure Audio Streaming Protocol:** Streams audio safely to the UI via `opentake-media://localhost/{asset_id}` byte-range protocol handler.
  - **Automatic Cue-to-Segment Placement:** Places generated speech segments at corresponding subtitle cue start times, detecting timing overruns.
  - **Rollback-Safe Regeneration:** Re-synthesizes speech with Piper TTS on demand with automatic fallback retention if generation fails.
  - **Interval-Based Diagnostics:** Efficient $O(N \log N)$ scan detecting segment overlaps, cue overruns, missing assets, unresolved voices, and large dialogue gaps.
  - **50-Step Undo/Redo & Project Persistence:** Deterministic in-memory command stack with lossless `.omwsub` project metadata embedding.
- **Professional AI Dubbing Duration Alignment (STEP 27 Implemented):**
  - **Grounded Timing Analysis:** Microsecond-precision comparison between speech sample duration and subtitle display windows.
  - **Deterministic AutoFit Planner:** Conservative tempo scaling (0.85x–1.15x policy; 1.25x max auto limit) with explainable strategy reporting and `NeedsReview` flags for extreme mismatches.
  - **Pitch-Preserving Audio Time-Stretching:** System FFmpeg `atempo` filter execution with sample-measured output verification and chained stages for wide tempo ranges.
  - **Sample-Accurate PCM Silence Padding:** Exact sample frame padding (`PadEnd`, `PadStart`, `Centered`) preserving dialogue start rhythm.
  - **Non-Destructive Asset Cache & Staging:** Content-addressed SHA-256 storage (`aligned_{hash}.wav`) with immutable source speech preservation and instant restoration.
  - **True A/B Preview & Inspector:** Dedicated inspector with duration comparison, manual tempo sliders (0.50x–2.00x), start offset nudge controls, and synchronized A/B audio player over `opentake-media://`.
- **Professional Original Audio Mixing & Multitrack Mixdown (STEP 28 Implemented):**
  - **Original Soundtrack & Speech-Only Mixdown:** Decodes source video soundtrack or executes clean speech-only mixdown when media lacks audio. Source files remain 100% byte-for-byte immutable.
  - **Sample-Accurate Speech Placement:** Places speech WAVs using FFmpeg `adelay` in sample units, respecting duration-aligned assets without double tempo adjustment.
  - **Dialogue Ducking Engine:** Automatically attenuates original soundtrack during active speech intervals with attack, hold, release, and $O(N \log N)$ interval merging to prevent volume pumping.
  - **Decibel Gain Staging & Limiter Protection:** Gain controls for original soundtrack, dubbing master, and per-track gains with `amix=normalize=0` and mastering limiter (`alimiter=limit=0.891`, -1.0 dBFS).
  - **Production Presets & Acoustic Analysis:** Verified presets (`Balanced`, `Voice Focus`, `Preserve Original`, `Custom`) and genuine measurements of Peak dBFS, RMS, LUFS, and clipping samples.
  - **Real Offline WAV Rendering & A/B Preview:** Async system FFmpeg mixdown producing durable 48 kHz stereo 16-bit PCM WAV assets with live A/B audition player and stale-mix invalidation.
- **Unified Media Export (Planned STEP 29)**
- **Multiplexed Soft Subtitle Export (Planned STEP 30):**
  - Uses the shared FFmpeg managed process engine to mux soft subtitle streams into output containers.

### 3.3 Shared Local AI Model Manager (STEP 18 Implemented)
**Objective:** Provide a unified, offline-first local AI model repository shared between Localization Studio and the future AI Shorts Studio.

- **Unified Model Repository:** A single local filesystem cache located at `~/.local/share/open-media-workbench/ai-models` serving all workspaces without duplicating multi-gigabyte files.
- **Small Installer Invariant:** Zero bundled weights. The standard application distribution package remains small (<100MB target for Linux).
- **Curated & Verified Catalog:** Upstream Git LFS artifacts with pinned SHA-256 digests (Whisper Tiny/Base for speech-to-text, Piper Lessac for speech synthesis, Qwen 2.5 0.5B for local text generation).
- **Explicit Consent & Staging:** Downloads require user confirmation with detailed disclosures (size, license, URL, disk requirements). Downloads stream in 64 KiB chunks with running SHA-256 calculation and cancellation cleanup.
- **Storage Protection:** Model ID sanitization, symlink attack rejection, atomic registry writes (`registry.json`), and in-use locking to prevent model deletion while active jobs are running.
- **Hardware Probing:** Best-effort detection via Linux `/proc/meminfo` and POSIX `statvfs` with honest disclosure of unverified GPU acceleration states.

---

## 4. Shared Player, Timeline & Background Job Services

To prevent code duplication, the two workspaces share core services:

```
┌─────────────────────────────────────────────────────────────┐
│                      Shared Services                        │
├──────────────────────────────┬──────────────────────────────┤
│ VideoPlaybackService         │ WaveformService              │
│ - opentake-media:// streaming│ - Multi-LOD peak extraction  │
│ - HTML5 video event sync     │ - Canvas rendering sync      │
│ - Frame-accurate stepping    │ - Playhead tracking          │
├──────────────────────────────┼──────────────────────────────┤
│ BackgroundJobManager         │ AuthorizationSession         │
│ - Async Tokio state machine  │ - Cryptographic path access  │
│ - PGID process tree killing  │ - Explicit file provenance   │
│ - Progress & ETA estimation  │ - Sandboxed media boundaries │
└──────────────────────────────┴──────────────────────────────┘
```

---

## 5. Data Ownership and Persistence Strategy

1. **Non-Destructive Guarantee:**
   - Input files (damaged media in Repair Studio, original footage in Localization Studio) are treated as **strictly read-only**.
   - All repairs, salvaged streams, synthesized audio, and exports are written to explicit user-designated target directories or sandboxed application scratch paths.
2. **Project Data Format (`.omw` / `.opentake`):**
   - Project definitions are serialized as human-readable, schema-versioned JSON.
   - Atomic disk writes (`fsync` to temporary file before atomic renaming) ensure resilience against sudden power loss.
3. **Workspace Preferences:**
   - Persisted in client `localStorage` with validated fallbacks, schema versioning, and layout reset capabilities.

---

## 6. Workspace Isolation and Dependency Boundaries

To maintain long-term stability and clean separation of concerns:
- **Rust Core Independence:** `crates/opentake-core` contains zero UI logic and zero Tauri dependencies. It is compileable as a pure headless crate and thoroughly testable via standard `cargo test`.
- **UI Decoupling:** Repair Studio and Localization Studio viewports reside in separate directory trees (`src/workspaces/repair/` and `src/workspaces/localization/`), sharing common components from `src/components/ui/` and `src/components/layout/`.
- **Hardware Acceleration:** Future hardware-accelerated decode/encode pipelines interface through clean trait abstractions in `opentake-core::media`.

---

## 7. Deferred Internal Identifier Migration Rationale

During this STEP 10 migration, several internal identifiers have been intentionally preserved:

| Identifier | Current Value | Preservation Rationale |
| :--- | :--- | :--- |
| **Tauri Bundle Identifier** | `org.opentake.app` | Changing bundle ID immediately changes OS app data paths (`~/.local/share/org.opentake.app`), invalidating existing cached waveforms and user preferences. |
| **Custom URI Protocol** | `opentake-media://` | Protocol scheme is registered in WebKitGTK and referenced in CSP rules; migrating must occur in synchronized lockstep with frontend media URLs. |
| **Core Crate Name** | `opentake-core` | Renaming crate generates churn across Cargo.lock and test imports; deferred until feature modules are established. |
| **Desktop Crate Name** | `opentake_desktop` | Preserves build scripts, Tauri CLI configurations, and binary output targets. |
| **Storage Key** | `opentake_workspace_preferences_v1` | Preserves existing user layout customizations across app launches. |

These identifiers will be scheduled for a seamless migration pass once workspace feature parity is achieved, using an automated preference/cache migrator.

---

## 8. Compatibility & Upgrade Strategy

1. **Deterministic Test Gates:**
   - Every commit must pass all existing Rust unit, property, and integration tests (83 tests) and TypeScript tests (16 tests + 6 workspace tests).
2. **Schema Upgraders:**
   - Any future modifications to saved project structures or cache headers will provide backward-compatible deserializers with automated migration paths.
3. **Graceful Degraded Runtime:**
   - If local AI models or hardware encoders are unavailable, the application gracefully alerts the user while keeping standard media playback and manual editing fully operational.

---

## 9. Workspace Domain Model & Lifecycle (STEP 11)

### 9.1 Typed Workspace Identities
Workspaces are strictly typed as:
```typescript
type WorkspaceId = "repair" | "localization";
```
Each workspace is defined declaratively (`WORKSPACE_DEFINITIONS`) with its label, description, badge, and capability matrix specifying implementation status (`available`, `in_development`, `planned`) and roadmap milestone step.

### 9.2 State Retention & Lifecycle
- **Shared Media Ingestion & Authorization:** Both studios mount the canonical `MediaLibraryPanel` backed by the Rust media session. Assets imported in one studio remain authorized and immediately selectable in the other without duplicating memory buffers or session state.
- **Active Asset Persistence:** When the user switches between Repair Studio and Localization Studio, the active asset selection is preserved across viewports.
- **Independent Studio State:** Transient inspector views, diagnostic states, and recovery workflow step indices are scoped to each workspace. Switching tabs does not destroy in-progress work or reset unrelated panel splitters.
- **Resource Ownership:** Video decoder elements (`<video>`) and audio waveform canvas buffers unmount deterministically, preventing background playback leaks or GPU resource hogging.

### 9.3 Keyboard Navigation & Accessibility
- **W3C ARIA Tablist Pattern:** The workspace switcher in `AppHeader` implements standard `role="tablist"` with `role="tab"`, `aria-selected`, `aria-controls`, and `tabIndex` attributes.
- **Keyboard Traversal:** Users can cycle between studios using `ArrowLeft`/`ArrowRight`/`Home`/`End` within the switcher, or globally using non-conflicting dedicated shortcuts:
  - `Alt+1`: Switch to Repair Studio
  - `Alt+2`: Switch to Localization Studio
- **Input Guard:** Global shortcuts strictly ignore keystrokes when focus is inside an `<input>` or `<textarea>` element.

---

## 10. Functional Capabilities vs Deferred Milestones Matrix

| Capability | Studio | Status | Delivery Milestone |
| :--- | :--- | :--- | :--- |
| **Media Import & Probing** | Shared | **Available** | STEP 6 (Verified) |
| **Video Playback & Proxy Engine** | Shared | **Available** | STEP 7 (Verified) |
| **Multi-LOD Waveform Pipeline** | Shared | **Available** | STEP 8 (Verified) |
| **Canonical Non-Destructive EDL** | Shared | **Available** | STEP 9 (Verified) |
| **Dual-Workspace Shell & Switcher** | Shared | **Available** | STEP 11 (Verified) |
| **Container & Atom Diagnostics** | Repair | **Available** | STEP 12 (Verified) |
| **Lossless Video Recovery & Stream Salvage** | Repair | **Available** | STEP 13 (Verified) |
| **Reference-Based Reconstruction Engine** | Repair | **Available** | STEP 14 (Verified) |
| **A/B Repair Preview & Multi-Tier Validation** | Repair | **Available** | STEP 15 (Verified) |
| **Interactive Subtitle Editor (CRUD & EDL)** | Localization | **Available** | STEP 16–17 (Verified) |
| **Shared AI Model Manager** | Shared | **Available** | STEP 18 (Verified) |
| **Local Offline Whisper Transcription** | Localization | **Available** | STEP 19 (Verified) |
| **Word-Level Subtitle Alignment** | Localization | **Available** | STEP 20 (Verified) |
| **Subtitle Burn-In Video Export** | Localization | **Available** | STEP 21 (Verified) |
| **Local Neural Machine Translation** | Localization | **Available** | STEP 22–23 (Verified) |
| **Neural Speech Synthesis & Dubbing Mixing** | Localization | **Available** | STEP 24–28 (Verified) |
| **Subtitle & Dubbed Multiplex Export** | Localization | *Planned* | STEP 29–31 |

