# Open Media Workbench

> **Free, open-source, offline-first desktop media recovery and AI localization studio.**

[![Status](https://img.shields.io/badge/Status-STEP%2026%20AI%20Dubbing%20Timeline%20Complete-success.svg)](docs/reports/STEP_26_REPORT.md)
[![Architecture](https://img.shields.io/badge/Architecture-Dual%20Workspace-indigo.svg)](docs/architecture/OPEN_MEDIA_WORKBENCH.md)


Open Media Workbench is a free, open-source, offline-first desktop application designed for media recovery and local AI localization. Built with Tauri 2, Rust, and React 19, the workbench provides content creators, archivists, and video professionals with an uncompromising local-first studio that processes all media strictly on-device without cloud telemetry or mandatory accounts.

---

## 🏛️ Product Architecture: Dual Studio Model

Open Media Workbench unifies two specialized studio workspaces under a single high-performance desktop shell, sharing a common Rust media processing core:

1. **Repair Studio** *(Active Development — STEPS 12, 13, 14 & 15 Complete)*
   - **Video Corruption Diagnostics (STEP 12):**
     - **Container Identification:** Fast magic byte validation for MP4/MOV, MKV (EBML), and AVI (RIFF) with extension mismatch detection.
     - **ISO BMFF Structural Inspection:** Bounded, memory-safe atom parser (`ftyp`, `moov`, `mdat`, `moof`, `free/skip`) handling 32-bit, 64-bit extended sizes, and size-0 atoms without loading sample payloads into memory.
     - **Resilient Stream Probing:** FFprobe metadata inventory that handles unfinalized or missing headers gracefully without crashing.
     - **Quick vs Deep Scan Modes:** Instant structural inspection (<100ms) or bounded 30-second decode scan (`-f null -`) identifying non-monotonic DTS jumps, macroblock concealment, and corrupt NAL slices.
     - **Evidence-Based Findings:** Structured codes (`MP4_MOOV_MISSING`, `TIMESTAMP_NON_MONOTONIC_DTS`) with concrete observed evidence strings and confidence scores.
     - **Sanitized Diagnostic JSON Export:** Privacy-preserving report export with native save destination (scrubs internal user directory paths).
     - **Cooperative Cancellation:** Responsive background job management with leak-free child process termination.
   - **Video Recovery Pipeline (STEP 13 Implemented):**
     - **Lossless Container Remux:** Pure stream copy (`-c copy`) of readable elementary streams into freshly muxed, compliant containers (MP4, MKV, MOV, AVI) with `-movflags +faststart`.
     - **Recoverable Stream Salvage:** Conservative isolation of intact video or audio tracks when sister streams suffer from corruption or unreadable data (Video-Only, Audio-Only, or All Readable Tracks).
     - **Conservative Timestamp Rebuild:** Presentation timestamp regeneration (`+genpts`, `avoid_negative_ts make_zero`) resolving non-monotonic DTS/PTS jumps and audio/video desynchronization.
     - **Post-Recovery Bounded Validation:** Three-stage output verification checking non-empty file size, container recognition, stream loss detection, and bounded FFmpeg null-sink decode test.
     - **Secure Staging & Safe Finalization:** Recovery operations execute strictly in isolated, application-managed staging directories. Final output export enforces atomic no-clobber protection (preventing silent overwrites) and defends against symlink redirection attacks.
     - **Zero-Byte Source Mutation:** Original source media files are strictly read-only and never modified, renamed, or truncated under any circumstances.
     - **Recovered Asset Preview Integration:** Validated outputs are registered into the active media library as new authorized assets and immediately playable in the desktop preview player.
     - **Versioned Recovery Reports:** Structured JSON reports detailing processing status, stream inventories, observed errors, duration deltas, and known limitations.
   - **Interrupted MP4/MOV Reconstruction Engine (STEP 14 Implemented):**
     - **Reference-Based Reconstruction:** Recovers unfinalized or interrupted recordings missing the index atom (`moov`) by pairing the damaged file with an authorized healthy reference recording from the same device and configuration.
     - **Donor Compatibility Analysis:** Pre-flight inspection comparing container family, video codec, profile, resolution, frame rate, timebase, and audio channels before attempting reconstruction.
     - **Dual Independent Authorization:** Both damaged and reference media files must be independently authorized via native file dialogs and verified through `MediaAuthorizationRegistry`. Arbitrary filesystem paths from untrusted IPC callers are strictly rejected.
     - **Built-in Safe Rust Reconstructor:** Out-of-the-box native reconstruction engine that extracts donor SPS/PPS parameter sets, scans damaged `mdat` payloads for length-prefixed and Annex-B NAL units, synthesizes sample tables, and produces compliant, playable ISO BMFF containers without requiring external C++ tools or third-party binary downloads.
     - **Pluggable Engine Adapter:** Extensible architecture supporting both the native safe Rust engine and external `untrunc` CLI tools if discovered on system `$PATH`.
     - **Zero-Byte Source & Reference Immutability:** Pre- and post-recovery SHA-256 hashes confirm both the damaged source and reference recording remain 100% byte-identical.
     - **Bounded Output Validation:** Reconstructed candidates are inspected for container recognition, stream inventory, and bounded decode playback testing (`-f null -`).
   - **Professional Recovery Preview, Validation & Reports (STEP 15 Implemented):**
     - **Secure Staged Candidate Preview:** Preview recovered media safely prior to final export using `SourceProvenance::StagedRecoveryCandidate` and opaque custom protocol tokens (`opentake-media://`) without exposing staging filesystem paths or expanding Tauri capabilities.
     - **Multi-Mode Before/After Comparison Player:** Side-by-side synchronized comparison (`Original | Recovered`) with linked or independent seek modes, as well as single recovered-only and original-only views. Unplayable originals are handled honestly: metadata and diagnostic findings remain visible while recovered candidate plays independently.
     - **Advanced Multi-Tier Output Validation:** Distinguishes `MetadataOnly`, `SampledDecode` (with explicit disclosure of sample coverage), and user-initiated on-demand `FullDecode` (verifying 100% of bitstream frames without unbounded automatic execution).
     - **Timeline Difference & Integrity Review:** Compares stream durations and start offsets using verified timestamp evidence; missing nominal durations are never fabricated.
     - **Persistent Local Recovery History:** Crash-safe, atomic versioned JSON ledger tracking up to 50 previous recovery attempts with status, validation levels, timestamps, and manual history clear action.
     - **Sanitized JSON & Standalone Offline HTML Reports:** User-initiated report export featuring stable machine-readable finding codes, strict Content-Security-Policy, offline embedded dark CSS, XSS escaping, and path privacy sanitization.

2. **Localization Studio** *(Active Development — STEPS 16, 17, 19, 20, 21, 22, 23, 24, 25 & 26 Implemented)*
   - **Canonical Subtitle Domain Model (STEP 16 Implemented):**
     - **Strict Microsecond Timestamps:** Microsecond-accurate integer representation (`MediaTimeUs`), preventing floating-point drift across playback ticks.
     - **Robust SubRip (.srt) Support:** Parsing and serialization supporting UTF-8 BOM, CRLF/LF line endings, multiline cues, and tolerant numbering.
     - **Standards-Aware WebVTT (.vtt) Support:** Parses WebVTT signatures, skips `NOTE` comments, inspects cue settings (`line:`, `position:`, `align:`, `size:`, `vertical:`), and extracts `<v Speaker>` voice tags into structured speaker attributes.
     - **Full Unicode & Bengali (বাংলা) Preservation:** Zero byte corruption or entity truncation when processing non-Latin alphabets and complex Unicode scripts.
     - **Automated Validation Diagnostic Engine:** Inspects tracks and documents for empty cues, inverted intervals, duplicate IDs, out-of-order cues, duration anomalies, and media bounds violations.
     - **Safe Local Import & Provenance:** Native `rfd` file picker enforcing `SourceProvenance::SubtitleFileDialog`, 10MB file size ceiling, and 20,000 cue caps without arbitrary filesystem writes or media mutation.
     - **Live Video Preview Overlay & Synchronized Seek:** Clean plain-text subtitle overlay rendered over the HTML5 preview player with Closed Captions (CC) toggle; click-to-seek cue list navigation powered by $O(\log N)$ binary search lookup.
     - **Safe Atomic Export & Project Persistence:** Export to compliant `.srt` and `.vtt` formats with temporary file rename guarantees; versioned `.omwsub` project document format.
   - **Professional Subtitle Timeline Editor (STEP 17 Implemented):**
     - **Visual Timeline Cue Blocks:** Interactive cue blocks on a dedicated `S1 Subtitle` track lane precisely aligned with the audio waveform canvas and horizontal time ruler.
     - **Interactive Timeline Manipulation:** Drag cue bodies to translate timing; drag left and right boundary handles to resize start and end times in integer microseconds.
     - **Multi-Target Snapping Engine:** Snaps cue boundaries to current playhead, neighboring cue start/end boundaries, and timeline grid with visual amber snap indicators and threshold controls.
     - **Split & Merge Workflows:** Split cues at playhead with clean Unicode/Bengali word and newline division; merge adjacent cues with deterministic text concatenation and metadata preservation.
     - **Precise Timing Inspector:** Dedicated start/end timecode inputs (`HH:MM:SS.mmm`), duration display, configurable nudge buttons (-100ms, +100ms, -500ms, +500ms), and playhead sync buttons ("Start @ Playhead", "End @ Playhead").
     - **Inline Dialogue Editor:** Multiline text editor with live character counts, lines count, reading speed (chars/sec), and non-blocking readability warnings (>42 chars/line).
     - **Multi-Cue Batch Editing:** Ctrl/Cmd-click and Shift-click range selection, batch nudge timing shifts with atomic negative-time rejection, and batch deletion.
     - **Multi-Level Undo/Redo:** 50-step deterministic history stack with `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`, and `Ctrl/Cmd+Y` shortcuts, designed to never intercept native text input undo events.
     - **Autosave & Crash Recovery:** Dirty state tracking with debounced snapshots and automatic recovery prompts on launch protecting against abrupt crashes or power loss.
   - **Offline Whisper Transcription Engine (STEP 19 Implemented):**
     - **Genuine Local Speech-to-Text:** Real on-device transcription utilizing official `whisper.cpp` inference backends through secure managed subprocess isolation. Zero cloud APIs, paid speech recognition tokens, telemetry, or mandatory accounts.
     - **Shared AI Model Integration:** Reuses installed GGML models (`whisper-tiny`, `whisper-base`) directly from the shared STEP 18 AI Model Manager repository. Zero redundant model downloads.
     - **Audio Preprocessing Pipeline:** Secure non-shell FFmpeg transformation staging 16 kHz mono signed 16-bit PCM WAV with automatic cleanup and cooperative cancellation.
     - **Multilingual & English Model Discrimination:** Supports automatic language detection, English, Bengali (বাংলা), and 90+ Whisper languages on multilingual models; automatically disallows non-English transcription on English-only (`.en`) models.
     - **Canonical Subtitle Integration:** Parses structured runtime outputs into validated microsecond `SubtitleCue` segments, with non-destructive track addition to existing `.omwsub` project documents.
     - **Measured On-Device Speed:** Measured 770ms execution on CPU for 11-second audio test fixture (~14x realtime speed with Whisper Tiny).
   - **Word-Level Timestamp Alignment, Smart Segmentation & Karaoke Foundation (STEP 20 Implemented):**
     - **Genuine Word-Level Timing Extraction:** Extracts measured token offsets and acoustic probabilities directly from `whisper.cpp` structured output (`--output-json-full`) without external Python runtimes or cloud aligners.
     - **Explicit Provenance & Timing Quality:** Classifies timings as `WhisperNativeEstimate`, `ForcedAlignment`, `UserAdjusted`, or `Unaligned`, coupled with document-level `TimingQuality` ratings (`Aligned`, `Estimated`, `PartiallyAligned`, `Unavailable`).
     - **Smart Subtitle Segmentation:** Intelligent heuristic segmentation partitioning aligned words into broadcast-compliant subtitle cues guided by configurable policies: maximum characters per line (e.g. 42), max lines per cue (e.g. 2), maximum and minimum cue duration bounds (1.0s – 7.0s), reading speed (CPS), natural silence pause splits (e.g. 400ms), and sentence punctuation breaks.
     - **Unicode-Safe Bengali Segmentation:** Complete preservation of Bengali (বাংলা) grapheme clusters, combining marks, nukta, virama, and Dari sentence punctuation (`।`, `॥`) without erroneous mid-word boundary cuts.
     - **Evidence-Based Timing Refinement:** Reversible boundary snapping matching cue start and end times to contained first and last word timestamps with minimum duration padding.
     - **Live Word-Level Karaoke Preview:** Real-time visual word highlighting inside the video preview subtitle overlay following speech timestamps without `dangerouslySetInnerHTML`.
     - **Waveform Timeline Word Markers:** Scalable, zoom-responsive word boundary markers on the subtitle timeline track lane when zoomed in (`>= 5%` cue width).
     - **Interactive Alignment Review & Manual Correction:** Dedicated `WordAlignmentPanel` for inspecting words, adjusting millisecond timestamps, seeking audio to word boundaries, and non-destructively applying formatted tracks to the project.
     - **Linear Performance Scaling:** Measured 100 words in 0.275ms, 1,000 words in 1.714ms, and 10,000 words in 20.024ms.
     - **Preservation & Export Compatibility:** Losslessly persists word timing metadata in `.omwsub` project format, migrates legacy schemas without data loss, and exports clean, compliant `.srt` and `.vtt` caption files.
   - **Professional Subtitle Styling & Video Burn-In Export (STEP 21 Implemented):**
     - **Canonical Subtitle Style Domain:** Fully typed style configurations controlling typography (font family, size, weight, italic), colors (text, outline, shadow, background box, word highlight), outline thickness, shadow offset, background opacity, frame-relative positioning (BottomCenter, Center, TopCenter, etc.), and animation presets.
     - **8 Professional Built-in Presets:** Clean Minimal, Cinematic, Modern Outline, Social Bold, Karaoke Highlight, Documentary, Accessible High Contrast, and Soft Background Box, alongside user-defined custom presets persisted locally.
     - **Local System Font Discovery & Bengali Support:** Native font discovery (`fc-list`) with fallback defaults and explicit Bengali font detection (`Noto Sans Bengali`, `Noto Serif Bengali`).
     - **Secure ASS Subtitle Generation:** Advanced SubStation Alpha generation with centisecond timing rounding, safe brace and character escaping, dialogue formatting, and animation tags (`\fad`, `\fscx`).
     - **Real Word-Level Karaoke:** Timing-accurate `\kf` karaoke tags powered by genuine word timing from STEP 20.
     - **Live React Subtitle Preview:** Instant dynamic styling updates on the video overlay adhering to canonical styles with live karaoke word highlighting.
     - **Production FFmpeg Burn-In Video Pipeline:** Real video burn-in re-encoding via system FFmpeg with libass filter integration (`ass=` filter), structured non-shell arguments, resolution options, CRF quality presets, and audio policy handling.
     - **Multi-Stage Output Validation:** ffprobe-based container, stream, and frame decode verification ensuring captions are visibly burned in and not merely accompanied by metadata.
     - **Safe Staging & Source Immutability:** Rendering executes strictly into app-owned staging directories; finalized export requires native save destination; source media is byte-for-byte immutable (verified via SHA-256).
   - **Offline Multilingual Subtitle Translation Engine (STEP 22 Implemented):**
     - **Real Local CPU Inference:** Genuine on-device machine translation powered by the official `llama.cpp` CLI runtime executing curated compact instruction models (`Qwen 2.5 0.5B Instruct` 4-bit quantized GGUF, 468 MB, Apache-2.0, single-file artifact). Zero cloud APIs, paid API tokens, telemetry, or remote servers.
     - **Shared AI Model Manager Integration:** Seamless integration with the shared AI Model Manager (`~/.local/share/opentake/models` & `registry.json`) with strict SHA-256 integrity verification (`74a4da8c9f...`) and runtime model locking preventing deletion during active inference.
     - **Authoritative Language Pair Capabilities:** Verified language pair: English to Spanish (`en` -> `es`). Experimental pairs: English to Bengali (`en` -> `bn`), Spanish to English (`es` -> `en`), Bengali to English (`bn` -> `en`). Unsupported language pairs are strictly disabled in the UI.
     - **Strict Timing Preservation Invariant:** Every translated cue strictly preserves the exact microsecond start and end timestamps of the original cue (`target_start_us == source_start_us`, `target_end_us == source_end_us`). Cue counts and ordering are strictly maintained.
     - **Non-Destructive Workflow & Original Safety:** Original subtitle tracks remain 100% byte-identical and untouched. Translation creates a distinct NEW `SubtitleTrack`. Source-language word timestamp evidence is explicitly cleared on translated cues to prevent invalid word-level acoustic assumptions.
     - **Translation Review & Subtitle Editor Integration:** Interactive `TranslationPanel` with track selection, language pair validation, runtime readiness indicators, live progress reporting, cancellation, side-by-side original/translated comparison, and readability warnings (>84 chars/line, >25 CPS).
     - **Export & Styling Compatibility:** Translated subtitle tracks can be manually edited in the timeline editor (STEP 17), styled and burned in to video (STEP 21), saved to `.omwsub` project files, and exported as standard SubRip (`.srt`) or WebVTT (`.vtt`).
     - **High-Speed On-Device Performance:** Measured 2.76 seconds total inference time on standard CPU for a multi-sentence subtitle track at ~255 t/s prompt evaluation and ~52 t/s text generation.
   - **Professional Bilingual Subtitle Review & Glossary Management (STEP 23 Implemented):**
     - **Side-by-Side Bilingual Editor:** Dual-column review panel displaying read-only source subtitles alongside in-line editable translated subtitles with synchronized playhead seeking upon cue selection.
     - **Deterministic Terminology Protection:** Local glossary management with protected brand names and proper nouns (e.g. `Open Media Workbench`, `FFmpeg`, `llama.cpp`), case sensitivity preferences, and Unicode word-boundary matching preventing false substring replacements.
     - **Translation Placeholder Masking:** Pre-translation token masking (`__OMW_TERM_0__`) and post-translation restoration verifying placeholder integrity and rejecting dropped or duplicated tokens.
     - **Approval Invalidation Invariant:** State machine transitioning through `Pending`, `NeedsReview`, `Reviewed`, `Approved`, and `Rejected`; any manual edit to an approved cue immediately invalidates approval to `NeedsReview` and clears its SHA-256 fingerprint.
     - **Rule-Based Quality Diagnostics:** Automated detection of missing translations, empty target text, excessive reading speed (>25 CPS), line length violations (>84 chars), timing deltas, and altered protected terminology.
     - **100% Local Translation Memory:** Exact-match normalized memory stored locally in `~/.local/share/opentake/review/translation_memory.json` with usage counters, one-click application during review, entry deletion, and complete clearing controls.
     - **Atomic Batch Review Operations:** Multi-row selection with batch Mark Reviewed, batch Approve, and batch Reject, coupled with review progress statistics and status/issue filtering.
     - **Review Gate & Persistence:** Full review documents embedded in canonical `SubtitleDocument.metadata` under `translationReview` across `.omwsub` project save/reopen cycles; optional review gate alerting users before exporting tracks with unresolved quality findings.
   - **Offline AI Text-to-Speech Engine (STEP 24 Implemented):**
     - **Real Local CPU Voice Synthesis:** Native offline speech inference powered by Piper TTS (`v2023.11.14-2`, MIT) executing compact ONNX models (`piper-en-lessac-medium`, 22,050 Hz 16-bit mono PCM). 100% offline, zero cloud APIs, no telemetry, no mandatory accounts.
     - **Script-to-Speech & Subtitle-to-Speech Workflows:** Enter or paste freeform text scripts with grapheme/Unicode-safe sentence chunking, or select existing SubtitleTracks to synthesize audio segments per cue.
     - **Non-Destructive Timing Delta Preservation:** Measures genuine audio duration from samples, calculating and reporting signed timing differences (`timing_difference_us`) against subtitle display intervals. Zero silent time-stretching, zero truncation, and zero mutation of source subtitle tracks.
     - **Integrated Audio Preview & Safe WAV Export:** HTML5 audio player previewing individual segments or full synthesized sequences; atomic no-clobber WAV export with format and silence validation.
     - **Shared AI Model Manager Multi-Artifact Integration:** Extended catalog supporting multi-file voice models (`.onnx` weights + `.onnx.json` config) with strict SHA-256 verification and in-use model locking.
     - **Future Dubbing Foundations:** Generates audio segment contracts preserving cue identifiers, source language, timestamps, and acoustic metadata for future Dubbing Timeline (STEP 26), Duration Alignment (STEP 27), and Audio Mixing (STEP 28).
   - **Professional Offline AI Voice Library (STEP 25 Implemented):**
     - **Verified Voice Model Discovery:** Curated catalog of verified offline voice models (`piper-en-lessac-medium`, `piper-en-libritts-high`, `piper-es-sharvard-medium`, `piper-bn-ibrahim-medium`) with display names, model IDs, quality tiers, sizes, and explicit licensing details.
     - **Search & Standards-Based Filtering:** Search voices by name or model; filter by ISO 639-1 language, BCP 47 locale, and installation status. Languages only appear selectable when a verified voice exists.
     - **Server-Side Compatibility Engine:** Multi-stage server-side validation evaluating catalog membership, installation state, artifact completeness (`.onnx` + `.onnx.json`), SHA-256 integrity, runtime presence, language support, and speaker ID boundaries.
     - **Genuine Offline Voice Preview:** Synthesizes authentic 16-bit PCM WAV audio using local Piper inference on CPU without generic sample substitution, fabricated waveforms, or cloud APIs. Features HTML5 audio playback with pause, seek, and accurate audio duration reporting.
     - **Side-by-Side Voice A/B Comparison:** Side-by-side comparison between two distinct, installed, and ready voices on identical sample text. If only one voice is ready, honestly informs user to install another voice via Model Manager.
     - **Bounded LRU Preview Cache:** Content-addressed preview cache (`voice_id:model_hash:speaker:settings:text_hash`) bounded to 50 files and 50 MB with automatic LRU eviction, model invalidation, and explicit clear cache action.
     - **Voice Preferences & Favorites:** Persistent favorites, recents, global preferred default voice, and last selected speaker variant saved atomically in `voice_preferences.json`.
     - **Track-Level Voice & Speaker Assignments:** Non-destructively binds voices and multi-speaker variants to subtitle tracks and establishes project defaults in `SubtitleDocument.metadata.voiceAssignments` without altering subtitle text or timestamps.
     - **Text-to-Speech Integration:** Seamless canonical voice synchronization between Voice Library, `LocalizationInspector`, and `TextToSpeechPanel`.
   - **Professional AI Dubbing Timeline (STEP 26 Implemented):**
     - **Non-Destructive Multi-Track Audio Editor:** Dedicated Dubbing Timeline workspace in Localization Studio displaying reference video, original audio, source subtitles, translated subtitles, and AI speech tracks.
     - **Microsecond Timeline Coordinates:** Integer microsecond (`MediaTimeUs`) timeline positioning, trim points (`source_in_us`, `source_out_us`), and duration arithmetic preventing rounding drift.
     - **Authentic Waveform Peak Extraction:** Safe 16-bit PCM WAV decimation extracting real min/max peak bins for trimmed clip bounds without fabricated shapes or decoding whole files on render.
     - **Full Non-Destructive Operation Suite:** Move segments, trim start (head trim), trim end (tail trim), split at playhead into adjacent non-overlapping segments, duplicate, delete, and adjust gain without mutating underlying audio files.
     - **Durable Managed Audio Storage:** Automatically stores generated dialogue WAV assets in `~/.local/share/opentake/dubbing/assets` with content hashing and reference counting. Survives application restarts without broken temporary paths.
     - **Secure Byte-Range Streaming Protocol:** Streams generated WAV audio to the UI using custom `opentake-media://localhost/{asset_id}` protocol handler with byte-range support, without exposing raw filesystem paths.
     - **Automatic Cue-to-Segment Placement:** Places generated speech segments at corresponding subtitle cue start times, computing exact duration and flagging timing overruns.
     - **Rollback-Safe Segment Regeneration:** Re-synthesizes speech with Piper TTS on demand; if generation fails, the previous working asset remains active without damaging manual trims.
     - **Comprehensive Diagnostic Engine:** Interval-based $O(N \log N)$ scan detecting segment overlaps, cue overruns, missing assets, unresolved voice assignments, and large dialogue gaps.
     - **50-Step Undo/Redo History:** In-memory command stack supporting undo/redo for move, trim, split, delete, duplicate, and track reordering.
     - **Shared Video Playhead Synchronization:** Authoritative synchronized playhead between video player, time ruler, subtitle cues, and audio clips with audition controls, mute, and solo.
     - **Crash-Safe Project Persistence:** Stores complete dubbing timeline projects atomically to disk and embeds state losslessly into `SubtitleDocument.metadata.dubbingProject` for `.omwsub` project files.
   - **Professional AI Dubbing Duration Alignment (STEP 27 Implemented):**
     - **Grounded Speech Duration Analysis:** Compares genuine synthesized speech sample durations against subtitle window boundaries using integer microsecond precision.
     - **Deterministic AutoFit Planner:** Conservative default tempo policy (0.85x–1.15x; max automatic limit 1.25x); exact fits preserve 1.0x tempo, shorter speech uses silence padding, moderate overruns are accelerated, and extreme overruns (>1.25x or >2.0x) are flagged as `NeedsReview`.
     - **Pitch-Preserving Tempo Engine:** System FFmpeg `atempo` filter execution with chained stages for extended ranges, structured shell-less execution, and sample-aware duration measurement.
     - **Sample-Accurate Silence Padding:** Pure 16-bit PCM sample buffer padding supporting `PadEnd`, `PadStart`, and `Centered` modes with zero filter rounding latency.
     - **True Non-Destructive Asset Preservation:** Original Piper TTS audio assets remain byte-identical; aligned outputs are stored atomically in durable content-addressed storage (`aligned_{hash}.wav`).
     - **Real-Time A/B Audio Audition:** True A/B playback comparing original speech vs processed aligned audio over secure `opentake-media://` protocol.
     - **Manual Alignment & Diagnostics Inspector:** Per-segment manual tempo sliders (0.50x–2.00x), silence padding selector, start offset nudge controls, and grounded sync diagnostics.
     - **Batch Alignment:** Concurrently analyzes, plans, and fits dialogue segments across tracks with partial-success tolerance and cancellation support.
     - **Undo/Redo & Project Persistence:** Fully integrated with 50-step edit history and atomic `.omwsub` project persistence.
   - **Original Audio Mixing & Multitrack Mixdown** *(Planned STEP 28)*
   - **Unified Media Export** *(Planned STEP 29)*
   - **Multiplexed Soft Subtitle Export** *(Planned STEP 30)*

3. **Shared Local AI Model Manager** *(Infrastructure Foundation — STEP 18 Implemented)*
   - **Unified Workspace Model Repository:** Shared local model cache (`~/.local/share/open-media-workbench/ai-models`) serving both Localization Studio and future AI Shorts Studio without duplicating multi-gigabyte files.
   - **Small Installer Strategy:** Zero bundled AI model weights. Clean release desktop binaries (~24 MB) with zero automatic downloads on application startup.
   - **Curated & Cryptographically Verified Catalog:** Pinned upstream Git LFS models with strict SHA-256 verification (Whisper Tiny/Base for speech-to-text, Piper Lessac for speech synthesis, and Qwen 2.5 0.5B for local text generation).
   - **Explicit User Consent & Transparency:** Clear confirmation modal disclosing exact artifact sizes, available disk space, model licenses, and official sources before initiating any download.
   - **Resilient Staging Pipeline:** Direct streaming network-to-disk writes in 64 KiB chunks with running SHA-256 computation, download cancellation cleanup, and size-overflow guardrails.
   - **Storage Safety & Anti-Traversal:** Model ID path sanitization, symlink attack rejection, atomic registry transactions (`registry.json`), startup orphan cleanup, and in-use locking preventing accidental deletion during active jobs.
   - **Hardware & Capability Discovery:** Safe non-privileged Linux hardware probing via `/proc/meminfo` (RAM) and POSIX `statvfs` (disk storage) with honest reporting of unverified GPU acceleration states.
   - **100% Offline Resilience:** Core Repair Studio, video playback, and subtitle editing operate with zero AI dependencies; previously installed models and catalog remain fully accessible offline.

---

## ⚠️ Important Recovery & Safety Guarantees

> [!IMPORTANT]
> **Zero-Byte Source Mutation Guarantee**
> All recovery, reconstruction, and subtitle operations execute strictly on read-only file descriptors. Output files are generated in isolated temporary staging locations or atomic swap paths. Source media files are **never** modified, overwritten, truncated, or moved.

> [!NOTE]
> **Lossless Stream Copy vs Re-encoding**
> The recovery and reconstruction engines prioritize lossless stream copy (`-c copy`) without generational loss. Damaged bitstream packets are preserved or isolated; content is never silently transcoded into degraded quality.

> [!TIP]
> **Reference Video Requirements**
> For reference-based reconstruction, use a working, finalized video recorded on the exact same camera or software (e.g. same smartphone, OBS profile, drone, or action camera) with matching resolution and frame rate. The donor file provides the missing SPS/PPS codec configuration and timescale required to rebuild the movie index.

> [!WARNING]
> **Unsupported Recovery Cases (Best-Effort Policy)**
> Open Media Workbench makes no universal recovery promises. Files with empty `mdat` payloads (0 bytes of media data), encrypted or DRM-protected content, or proprietary camera-specific raw bitstreams cannot be reconstructed.

> [!TIP]
> **Strict No-Clobber Export Safety**
> When saving recovered media or subtitle files to user storage, Open Media Workbench requires native OS file dialog confirmation, forbids writing over the original source or reference files, and rejects attempts to overwrite existing files without explicit user confirmation.

---

## ⚡ Current Project Status

> [!IMPORTANT]
> **Project State: STEP 24 Completed (Offline AI Text-to-Speech Engine: Real Local Voice Synthesis, Audio Preview, Voice Model Integration & WAV Export)**
> The dual-workspace desktop UI (STEP 11), corruption diagnostics engine (STEP 12), safe remux/salvage pipeline (STEP 13), reference-based reconstruction engine (STEP 14), recovery review/reporting pipeline (STEP 15), subtitle foundation (STEP 16), professional subtitle editor (STEP 17), shared AI model manager (STEP 18), offline Whisper transcription engine (STEP 19), word-level alignment (STEP 20), subtitle styling & burn-in export (STEP 21), offline translation engine (STEP 22), bilingual subtitle review & glossary management (STEP 23), and offline AI text-to-speech engine (STEP 24) are fully implemented and verified with **158 automated Rust tests** and **101 TypeScript unit tests**. Next milestone: **STEP 25**.


- **Target Release:** v0.1.0 Beta
- **Development Schedule:** ~30 days (engineering target)
- **Primary Development Environment:** Ubuntu Linux x86_64
- **Primary Tech Stack:** Tauri 2, Rust (Tokio), React 19, TypeScript, Tailwind CSS v4, FFmpeg/FFprobe

---

## 🚀 Developer Quickstart

### Prerequisites (Ubuntu / Debian Linux)
Ensure standard native build tools, FFmpeg, and WebKitGTK libraries are installed:
```bash
sudo apt update
sudo apt install -y build-essential curl pkg-config libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev ffmpeg
```

### Installation & Local Run
```bash
# 1. Install Node.js frontend dependencies
npm install

# 2. Run the native desktop application in development mode
npm run tauri dev

# 3. Run frontend standalone preview (browser only)
npm run dev
```

### Verification & Quality Checks
```bash
# Frontend typecheck, lint, and unit tests
npm run typecheck
npm run lint
npm test
npm run build

# Rust workspace compilation, unit tests, clippy, and binary build
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p opentake-desktop
```

### 💾 Disk Optimization & Build Cache Maintenance
To prevent `target/` directory bloat during local development across 15+ integration test suites, the workspace configures line-table debuginfo (`debug = 1`) for `[profile.dev]` and `[profile.test]` in root `Cargo.toml`. This preserves full source file and line number panic backtraces while reducing intermediate object and test binary sizes by ~60–70%.

To safely reclaim disk space from old test executables and incremental caches while preserving the latest release binary and bundle packages:
```bash
# Safely clean debug compilation cache and intermediate release artifacts:
rm -rf target/debug target/release/deps
```


---

## 📜 Licensing & Distribution

Open Media Workbench is distributed under the **GNU General Public License v3.0 or later (GPL-3.0-or-later)**.
- See [LICENSE](LICENSE) for the full license text.
- See [docs/architecture/LICENSING_AND_DISTRIBUTION.md](docs/architecture/LICENSING_AND_DISTRIBUTION.md) for licensing architecture and compliance invariants.
- See [docs/architecture/MP4_RECONSTRUCTION_ENGINE.md](docs/architecture/MP4_RECONSTRUCTION_ENGINE.md) for reconstruction engine architecture and licensing evidence.
