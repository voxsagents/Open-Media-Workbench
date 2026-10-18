# Open Media Workbench — Subtitle Foundation Architecture (STEP 16)

**Module:** `crates/opentake-core/src/localization/subtitles/`  
**Desktop IPC:** `src-tauri/src/commands/subtitles.rs`  
**Frontend Components:** `src/workspaces/localization/SubtitleTrackManager.tsx`, `src/components/editor/SubtitleOverlay.tsx`  
**Document Version:** 1.0.0  
**Status:** STEP 16 Completed  

---

## 1. Architectural Vision

In Open Media Workbench, the **Localization Studio** provides content creators, subtitlers, and archivists with an uncompromising, offline-first environment for multi-language captioning and localization. 

The subtitle foundation implemented in **STEP 16** establishes the authoritative, lossless subtitle substrate upon which upcoming interactive editing (STEP 17–18), local Whisper speech-to-text (STEP 19), neural translation (STEP 22), and voice dubbing (STEP 24–28) operate:

```
┌────────────────────────────────────────────────────────────────────────┐
│               Localization Studio UI (React 19)                        │
│  ┌──────────────────────────────┐    ┌──────────────────────────────┐  │
│  │   Preview Caption Overlay    │    │    Subtitle Track Manager    │  │
│  │ - HTML5 video synchronization│    │ - SRT / WebVTT file import   │  │
│  │ - Safe Unicode/Bengali text  │    │ - Multi-track management     │  │
│  │ - Closed Captions (CC) toggle│    │ - Click-to-seek cue list     │  │
│  │ - Bounded binary-search sync │    │ - Validation notices drawer  │  │
│  └──────────────┬───────────────┘    └──────────────┬───────────────┘  │
└─────────────────┼───────────────────────────────────┼──────────────────┘
                  │ Tauri IPC (`opentake-core` IPC)  │
┌─────────────────┴───────────────────────────────────┴──────────────────┐
│                   Tauri 2 Native Desktop Layer                         │
│  ├── Native rfd File Choosers (`.srt`, `.vtt`, `.omwsub`)              │
│  ├── Security Authorization (`SourceProvenance::SubtitleFileDialog`)   │
│  └── Atomic Crash-Resistant Persistence (`tempfile` + rename)          │
└─────────────────┬───────────────────────────────────┬──────────────────┘
                  │ Headless Rust Core Domain                            │
┌─────────────────┴───────────────────────────────────┴──────────────────┐
│                 opentake-core::localization::subtitles                 │
│  ├── model.rs          (Canonical microsecond domain data structures)  │
│  ├── srt.rs            (SubRip parser & deterministic serializer)      │
│  ├── webvtt.rs         (WebVTT standards parser, settings & cleaner)   │
│  ├── validation.rs     (Multi-level automated diagnostic inspector)   │
│  ├── serialization.rs  (Unified format dispatcher & detectors)        │
│  └── persistence.rs    (Versioned atomic JSON document store)          │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Canonical Subtitle Domain Model

All temporal values in Open Media Workbench subtitle tracks are represented using the integer microsecond time primitive (`MediaTimeUs`), preventing the timing drift, frame-jitter, and compounding truncation errors inherent in floating-point seconds.

### 2.1 Domain Invariants

1. **Non-Negative Start Times:** $t_{\text{start}} \ge 0$. Negative start timestamps are strictly rejected.
2. **Positive Duration:** $t_{\text{end}} > t_{\text{start}}$. A cue cannot end before or at its start time.
3. **Multiline Unicode Preservation:** Full support for non-Latin scripts (e.g. Bengali `বাংলা`, Arabic, Japanese, emojis) without character loss or mojibake.
4. **Stable Identifiers:** Every cue retains a UUID-based identifier that remains stable across persistence round-trips.

### 2.2 Data Structures

```rust
pub struct SubtitleCue {
    pub id: String,
    pub start_us: MediaTimeUs,
    pub end_us: MediaTimeUs,
    pub text: String,
    pub speaker: Option<String>,
    pub settings: Option<WebVttCueSettings>,
    pub raw_identifier: Option<String>,
    pub custom_metadata: HashMap<String, String>,
    /// Word-level timing alignment metadata (STEP 20 extension).
    /// Serialized with #[serde(default, skip_serializing_if = "Option::is_none")]
    /// to preserve 100% backward and forward schema compatibility.
    pub words: Option<Vec<AlignedWord>>,
}

pub struct SubtitleTrack {
    pub id: String,
    pub language: String,
    pub display_name: String,
    pub source_format: SubtitleFormat,
    pub cues: Vec<SubtitleCue>,
    pub is_enabled: bool,
    pub is_visible: bool,
    pub schema_version: u32,
}

pub struct SubtitleDocument {
    pub schema_version: u32,
    pub id: String,
    pub associated_media_asset_id: Option<String>,
    pub tracks: Vec<SubtitleTrack>,
    pub active_track_id: Option<String>,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub metadata: HashMap<String, String>,
}
```

### 2.3 Persistence & Export Boundaries for Word-Level Metadata
- **Native Document (`.omwsub`):** The canonical project format stores full word-level metadata losslessly, including stable word IDs, start and end microseconds, source segment references, punctuation metadata, and timing provenance.
- **SubRip (`.srt`) Export:** SRT format does not support word-level alignment metadata; cues are serialized as standard plain-text subtitle blocks.
- **WebVTT (`.vtt`) Export:** WebVTT exports valid cues and settings. Standard WebVTT serializers do not invent non-standard tags for arbitrary word metadata.
- **Backward Compatibility:** Existing `.omwsub` documents created prior to STEP 20 deserialize seamlessly with `words: None`.

---

## 3. SubRip (.srt) and WebVTT (.vtt) Specifications

### 3.1 SubRip (.srt) Parsing & Serialization

- **Tolerant Ingestion:** Accepts UTF-8 with or without Byte Order Mark (`\u{feff}`), normalizes Windows (`\r\n`), Mac (`\r`), and Unix (`\n`) line endings, and tolerates missing or out-of-order sequence index numbers.
- **Timestamp Arithmetic:** Supports standard comma millisecond delimiters (`00:01:23,456`) and dot delimiters (`00:01:23.456`). Trailing coordinate tokens (e.g. `X1:000 Y1:000`) are cleanly stripped.
- **Deterministic Serialization:** Sequential cue indices (1, 2, 3...) formatted chronologically with exact nearest-millisecond rounding (`(us + 500) / 1000`).

### 3.2 WebVTT (.vtt) Standards Parsing

- **Signature Header:** Requires valid `WEBVTT` header line.
- **Block Classification:** Cleanly skips `NOTE` comment blocks without error. Transparently bypasses `STYLE` and `REGION` blocks with informative compatibility warnings.
- **Short Timestamps:** Supports both full `HH:MM:SS.mmm` and compact `MM:SS.mmm` syntax.
- **Cue Settings:** Preserves positioning attributes: `line:`, `position:`, `size:`, `align:`, and `vertical:`.
- **Safe Text Extraction:** Extracts `<v Speaker>` voice tags into structured speaker attributes while sanitizing HTML markup (`<b>`, `<i>`, `<c>`). HTML entities (`&amp;`, `&lt;`, `&gt;`, `&nbsp;`) are unescaped to pure Unicode.

---

## 4. Structured Validation Diagnostic Engine

The validation engine inspects tracks without mutating content:

| Finding Code | Severity | Description |
|---|---|---|
| `EMPTY_TRACK` | Warning | Track has no subtitle cues |
| `EMPTY_CUE_TEXT` | Warning | Cue contains empty or whitespace-only text |
| `INVALID_TIME_RANGE` | Error | End time is less than or equal to start time |
| `NEGATIVE_START_TIME` | Error | Start time is less than zero |
| `UNSORTED_CUES` | Warning | Cue start time precedes previous cue start time |
| `OVERLAPPING_CUES` | Warning | Cue start time falls before previous cue end time (permitted for concurrent speakers) |
| `DUPLICATE_CUE_ID` | Warning | Multiple cues share the identical ID |
| `EXCESSIVE_DURATION` | Warning | Cue duration exceeds 30 seconds |
| `EXTREMELY_SHORT_DURATION` | Info | Cue duration is shorter than 100 milliseconds |
| `OUT_OF_MEDIA_BOUNDS` | Warning | Cue timestamp exceeds verified container duration |

---

## 5. Security & File Provenance

1. **Origin Verification:** Subtitle files imported via native file pickers are registered in `MediaAuthorizationRegistry` with `SourceProvenance::SubtitleFileDialog`.
2. **Resource Boundaries:** Hard ceilings are enforced:
   - Maximum file size: **10 MB** (`MAX_SUBTITLE_FILE_SIZE`).
   - Maximum cue count: **20,000 cues** per track (`MAX_SUBTITLE_CUE_COUNT`).
3. **No Arbitrary Filesystem Writes:** React cannot pass arbitrary paths to write files. All exports and saves execute via native OS dialogs with atomic temporary file swapping (`.tmp` -> `rename`).
4. **Media Immutability:** Video files are 100% read-only. Subtitles are bound by asset ID without embedding or touching video bytes.

---

## 6. Timeline Synchronization & Performance

- **Authoritative Coordinate Space:** Cues are authored in **Source Media Time** (`MediaTimeUs`).
- **Binary Search Lookup ($O(\log N)$):** During video playback (up to 60 FPS), the UI uses binary partition search rather than $O(N)$ linear scans to identify active cues, ensuring buttery-smooth laptop performance with files containing thousands of cues.
- **Safe Caption Overlay:** Captions are rendered as pure React text elements with text shadows and backdrop blur. `dangerouslySetInnerHTML` is strictly prohibited.

---

## 7. Interactive Subtitle Editing Architecture (STEP 17)

The foundations laid in STEP 16 are extended into a full interactive timeline editor in STEP 17. See [SUBTITLE_EDITOR.md](SUBTITLE_EDITOR.md) for complete details on:
- The canonical `SubtitleOperation` domain engine in Rust and TypeScript
- Direct visual manipulation on the audio waveform canvas
- Snapping algorithms and visual alignment
- Multi-level history and crash recovery architecture

---

## 8. AI Transcription Integration (STEP 19)

Offline Whisper speech recognition outputs convert directly into canonical `SubtitleTrack` objects:
- **Canonical Microsecond Mapping:** Whisper millisecond offsets and timestamps (`HH:MM:SS.mmm`) map deterministically into integer `MediaTimeUs` values.
- **Non-Destructive Track Addition:** Generated subtitle tracks append to the active `SubtitleDocument` as a new track (`append_track_to_document`), preserving all existing manual tracks and user edits.
- **Immediate Editing and Export:** Generated cues are instantly editable using the STEP 17 visual editor, subject to the same validation rules, and exportable to compliant `.srt` and `.vtt` formats.

---

## 9. Word Alignment & Smart Segmentation (STEP 20)

See [WORD_ALIGNMENT_AND_SEGMENTATION.md](WORD_ALIGNMENT_AND_SEGMENTATION.md) for full details on:
- Measured word-level token timestamps and acoustic probabilities from Whisper.
- Heuristic smart segmentation policies (reading speed, CPS, natural silence pauses, punctuation).
- Evidence-based timing refinement and live word-level karaoke previews.

---

## 10. Subtitle Styling & Burn-In Export (STEP 21)

See [SUBTITLE_STYLING_AND_BURNIN.md](SUBTITLE_STYLING_AND_BURNIN.md) for full details on:
- Canonical `SubtitleStyle` model covering typography, colors, borders, shadows, background boxes, positioning, and animation presets.
- 8 professional built-in presets and atomic local custom preset persistence.
- Safe Advanced SubStation Alpha (`.ass`) subtitle script generation with injection-resistant escaping and centisecond rounding.
- Real FFmpeg burn-in video re-encoding pipeline with `libass` integration and frame decode validation.

---

## 11. Offline Multilingual Subtitle Translation (STEP 22)

See [OFFLINE_TRANSLATION_ENGINE.md](OFFLINE_TRANSLATION_ENGINE.md) for full details on:
- On-device local inference via `llama.cpp` using compact `Qwen 2.5 0.5B Instruct` 4-bit quantized GGUF weights.
- Strict timing preservation invariant: `target_start_us == source_start_us` and `target_end_us == source_end_us`.
- Non-destructive track addition appending a new target-language `SubtitleTrack` to the active `SubtitleDocument`.
- Word alignment evidence isolation: source-language word-level token timestamps are cleared on translated cues to prevent invalid acoustic alignment assumptions.
- Persistence in `.omwsub` and direct export to SubRip (`.srt`) and WebVTT (`.vtt`).

---

## 12. Bilingual Review, Glossaries & Translation Memory (STEP 23)

See [BILINGUAL_REVIEW_AND_GLOSSARIES.md](BILINGUAL_REVIEW_AND_GLOSSARIES.md) for full details on:
- Side-by-side bilingual review workspace pairing source and target `SubtitleTrack` cues with synchronized video playback.
- Stable cue pairing using `source_cue_id` and timing without brittle array indexing.
- Deterministic review state machine (`Pending`, `NeedsReview`, `Reviewed`, `Approved`, `Rejected`) with strict approval invalidation upon text/timing modifications.
- Local project and global terminology glossaries with whole-word matching and brand protection masking.
- Privacy-first local Translation Memory (TM) with exact matching, one-click reuse, and user deletion controls.

---

## 13. Offline AI Text-to-Speech Engine (STEP 24)

See [OFFLINE_TTS_ENGINE.md](OFFLINE_TTS_ENGINE.md) for full details on:
- Subtitle-to-Speech synthesis generating dedicated `TtsAudioSegment` records per cue.
- Source cue identity preservation: segments retain `cue_id`, `cue_start_us`, `cue_end_us`, and `cue_duration_us`.
- Real timing discrepancy measurement: calculates signed `timing_difference_us = (duration_us as i64) - (cue_duration_us as i64)` without silent time-stretching or truncation.
- Strict non-destructive behavior: source subtitle tracks and cues are immutable and never modified by TTS synthesis.
- Audio preview and WAV export for downstream dubbing pipelines (STEPS 26–28).

---

## 14. Project Voice Assignments & Multi-Speaker Variants (STEP 25)

See [AI_VOICE_LIBRARY.md](AI_VOICE_LIBRARY.md) for full details on:
- Track-level voice assignments and project default voices persisted within `SubtitleDocument.metadata["voiceAssignments"]`.
- Multi-speaker variant selection persisted per track assignment.
- Absolute cue immutability: assigning voices alters zero subtitle cues or timestamps.
- Forward compatibility: establishes clear assignment contracts for STEP 26 timeline construction.

---

## 15. Professional AI Dubbing Timeline (STEP 26)

See [DUBBING_TIMELINE.md](DUBBING_TIMELINE.md) for full details on:
- Non-destructive multi-track audio segment timeline editing coupled to subtitle cues.
- Real 16-bit PCM WAV peak extraction without full-file decoding or decorative waveforms.
- Move, head trim, tail trim, split, duplicate, and delete operations on speech segments.
- Complete subtitle cue and media file immutability: subtitle text, display intervals, and source media files remain 100% unaltered.
- Complete dubbing project state persisted in `.omwsub` project files via `SubtitleDocument.metadata["dubbingProject"]`.
