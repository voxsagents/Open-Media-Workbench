# Subtitle Editor Architecture: Timing, Manipulation, Snapping & History

**Document Version:** 1.0.0  
**Status:** Implemented in Step 17  
**Scope:** Open Media Workbench — Localization Studio  

---

## 1. Overview & Architecture Philosophy

The subtitle editor in Open Media Workbench turns the canonical data models established in STEP 16 into a direct-manipulation, high-precision timeline editing environment. Built around microsecond integer precision (`MediaTimeUs`), the editor provides non-destructive cue manipulation, multi-target snapping, interactive waveform alignment, and deterministic multi-level history.

### Core Architectural Principles
1. **Zero Floating-Point Drift:** All internal timing logic, interval calculations, and persistence store time exclusively as 64-bit integer microseconds (`MediaTimeUs`).
2. **Unified Domain Contract:** The exact same operation rules (Add, Delete, UpdateText, SetTiming, Move, Resize, Split, Merge, BatchShift, Duplicate) are implemented with identical validation rules in the Rust processing core (`opentake-core::localization::subtitles::operations`) and the client domain engine (`src/lib/subtitleOperations.ts`).
3. **Zero-Byte Media Immutability:** Editing subtitles never touches or mutates the underlying video or audio containers.
4. **Non-Destructive History:** Edits are tracked in a multi-level transaction history, isolating native text input typing events from document-level timeline edits.
5. **Direct Manipulation with Waveform Alignment:** Subtitles render on an interactive `S1 Subtitle` lane positioned directly beneath the video and audio waveform canvases, sharing the exact same horizontal time ruler and viewport transformations.

---

## 2. Coordinate Spaces & Time Mapping

The editor operates across four explicitly mapped coordinate spaces:

```
[ Display Pixels (px) ]
         ↕ (Viewport Zoom & Scroll: x_px = (t_us - start_us) / span_us * width_px)
[ Timeline Track Time (MediaTimeUs) ]
         ↕ (1:1 Authoritative Mapping in Step 17)
[ Source Media Time (MediaTimeUs) ]
         ↕ (EDL Invariant: t_source >= 0)
[ Canonical ISO Media Container ]
```

### Display Pixel Coordinate Conversion
Given timeline container width $W_{px}$, visible viewport interval $[T_{start}, T_{end}]$ in microseconds, and timestamp $t$ in microseconds:

$$x_{px} = \frac{t - T_{start}}{T_{end} - T_{start}} \times W_{px}$$

Inverse mapping for pointer events:
$$t = T_{start} + \frac{x_{px}}{W_{px}} \times (T_{end} - T_{start})$$

---

## 3. Atomic Edit Operations (`SubtitleOperation`)

Editing operations are strongly typed and serialized over Tauri 2 IPC:

| Operation | Rust & TS Payload | Invariant Checks |
|---|---|---|
| `AddCue` | `{ trackId, cue }` | $start \ge 0$, $end > start$, sort maintained |
| `DeleteCue` | `{ trackId, cueId }` | Validates cue existence; removes cue |
| `DeleteCues` | `{ trackId, cueIds }` | Batch removes specified cue IDs |
| `UpdateCueText` | `{ trackId, cueId, text }` | Unicode preserved; multiline preserved |
| `UpdateCueSpeaker` | `{ trackId, cueId, speaker }` | Updates optional speaker voice tag |
| `SetCueTiming` | `{ trackId, cueId, startUs, endUs }` | $0 \le start < end$; re-sorts track cues |
| `MoveCue` | `{ trackId, cueId, deltaUs }` | $start + \Delta \ge 0$; overflow checks |
| `ResizeCueStart` | `{ trackId, cueId, newStartUs }` | $0 \le newStart < end$ |
| `ResizeCueEnd` | `{ trackId, cueId, newEndUs }` | $newEnd > start$ |
| `SplitCue` | `{ trackId, cueId, splitTimeUs }` | $start < splitTime < end$; splits text at word/newline |
| `MergeCues` | `{ trackId, firstCueId, secondCueId }` | Extends time span; deterministic text concatenation |
| `ShiftSelectedCues` | `{ trackId, cueIds, deltaUs }` | **Atomic:** Rejects batch if ANY cue becomes negative |
| `DuplicateCue` | `{ trackId, cueId }` | Clones cue immediately adjacent with new UUID |
| `UpdateTrackMetadata` | `{ trackId, language?, displayName? }` | Updates language tag and display name |

---

## 4. Snapping Engine Architecture

The snapping engine dynamically guides drag and resize operations to prevent fractional misalignments:

### Snap Targets
1. **Current Playhead:** Snaps boundary directly to $playhead\_us$.
2. **Neighboring Cue Start Times:** Aligns start of current cue with start of neighboring cues.
3. **Neighboring Cue End Times:** Snaps start or end of current cue flush against adjacent cue end times (gapless transitions).
4. **Timeline Grid Intervals:** Snaps to ruler tick intervals based on zoom level.

### Threshold Calculation
Snap threshold is defined in display pixels (default: $8\text{px}$ to $10\text{px}$) and dynamically converted to microsecond threshold:

$$\text{Threshold}_{us} = \frac{\text{SnapThreshold}_{px}}{W_{px}} \times (T_{end} - T_{start})$$

When snapping occurs, an amber indicator vertical guide line is rendered with a high-contrast badge indicating target type.

---

## 5. Multi-Level Undo/Redo & Crash Recovery

### History Architecture
- **History Depth:** Bounded to 50 document snapshots (`MAX_UNDO_STACK_SIZE`).
- **Transaction Boundaries:** User interactions (e.g. pointer release after drag, explicit text commit on blur, split, merge, batch nudge) form discrete atomic history states.
- **Redo Invalidation:** Any new edit action clears the redo stack.
- **Text Input Protection:** Global shortcut listeners check `event.target`. When typing inside `<textarea>` or `<input>`, the browser's native text editing undo stack handles character edits without conflicting with document-level timeline state.

### Crash Recovery & Autosave Snapshots
- Local recovery snapshots are serialized to local storage under key `omw_subtitle_autosave_{assetId}`.
- Whenever an operation mutates the document, the recovery snapshot is saved with a timestamp and dirty flag.
- On launch or asset selection, if a recovery snapshot exists that is newer than the persisted project file, the user is presented with a non-intrusive recovery banner:
  - `Restore`: Instantly recovers the unsaved session into memory.
  - `Dismiss`: Clears the recovery snapshot.
- A clean manual project save (`.omwsub`) clears the autosave snapshot automatically.

---

## 6. Video Preview & Overlapping Subtitle Rendering

- **Multiple Overlapping Cues:** In standard film and broadcast subtitling, dual speakers frequently overlap for several hundred milliseconds. Rather than discarding or hiding overlapping cues, the partition point lookup algorithm returns all concurrently active cues.
- **Stacking Overlay:** `SubtitleOverlay` renders all concurrently active cues stacked vertically with semi-transparent dark backgrounds and text shadows.
- **Zero XSS:** Subtitle text is rendered strictly as plain React children (no `dangerouslySetInnerHTML`).

---

## 7. Automatic Subtitle Ingestion from Whisper Transcription (STEP 19)

The Subtitle Editor directly integrates with the offline Whisper transcription engine:
- **Direct Entry Point:** The Subtitle Track Manager toolbar features a "Transcribe" button that navigates directly to the dedicated AI Transcription panel.
- **Preview Before Ingestion:** Recognition results (detected language, duration, and cue preview) are displayed for user confirmation before adding to the document.
- **Non-Destructive Ingestion:** New tracks are appended with source format `nativeProject`, unique cue identifiers, and language tags without overwriting existing tracks.
- **Full Editor Interactivity:** Generated cues immediately populate the visual timeline track, audio waveform canvas, and cue inspector with split/merge, snapping, timecode nudging, undo/redo, and SRT/VTT export capabilities.

---

## 8. Word-Level Karaoke Highlighting & Waveform Markers (STEP 20)

### 8.1 Live Word Highlighting in Video Preview
- When cues contain `words: Option<Vec<AlignedWord>>` metadata, `SubtitleOverlay` renders words individually.
- During playback, the current playhead time (`currentTimeUs`) is compared against each word's `[start_us, end_us]` interval.
- Words falling within the active interval receive a distinct karaoke highlight (`bg-amber-400 text-black font-semibold rounded px-0.5`).
- Highlighting can be toggled on/off in the inspector panel (`enableKaraoke`).
- If word alignment is missing or disabled, the overlay falls back gracefully to standard full-cue text rendering.
- Text rendering strictly avoids `dangerouslySetInnerHTML` to prevent XSS vulnerabilities.

### 8.2 Zoom-Dependent Waveform Timeline Markers
- On the `SubtitleTimelineLane`, when the user zooms into the timeline such that a cue occupies $\ge 5\%$ of the timeline width, individual word boundaries are rendered as subtle vertical ticks along the top of the cue block.
- Hovering or inspecting word boundaries reveals word text and timing.
- The markers can be toggled on/off via `showWordMarkers`.
- Rendering is memory-efficient with no per-frame recomputation; coordinates are derived directly from the linear viewport transform.

---

## 9. Subtitle Styling & Burn-In Export Integration (STEP 21)

### 9.1 Professional Styling Panel & Overlay Customization
- The Localization Studio inspector features a dedicated **Styling** tab hosting `SubtitleStylingPanel`.
- Users can choose from 8 built-in professional style presets (Clean Minimal, Cinematic, Modern Outline, Social Bold, Karaoke Highlight, Documentary, Accessible High Contrast, Soft Background Box) or create and persist custom presets.
- Typography (font family with system font detection, font size, weight, italic), colors (text, outline, shadow, background box, karaoke word highlight), outline thickness, shadow offset, background opacity, and frame positioning are customizable in real-time.
- The `SubtitleOverlay` in `PreviewPanel` directly binds to the active `SubtitleStyle`, dynamically adjusting CSS typography, text-shadow offsets, container positioning, and background opacity with zero player teardown.

### 9.2 Burn-In Video Export
- The **Export** tab hosts `SubtitleBurnInPanel`, giving creators end-to-end video burn-in capabilities.
- Select source video, subtitle track, and styling preset.
- Choose export resolution (Original, 1080p, 720p, Custom), CRF visual quality, and audio stream handling (`copy` or re-encode to `aac`).
- Executes genuine FFmpeg re-encoding via `ass` filter and `libass` with real progress updates, cooperative cancellation, ffprobe post-validation, and safe staging to avoid corrupting source media.

---

## 10. Offline Multilingual Subtitle Translation Integration (STEP 22)

### 10.1 Dedicated Translation Panel
- Localization Studio includes a dedicated **Translate** tab (`TranslationPanel.tsx`) alongside Subtitles, Transcription, Styling, and Export.
- Users select an existing source subtitle track, choose target language (e.g. English -> Spanish), and select an installed verified local translation model (`Qwen 2.5 0.5B Instruct`).
- Provides pre-flight checks: model presence, file integrity, and runtime operational readiness (`llama.cpp`).
- Features interactive progress tracking, cooperative cancellation, side-by-side original/translated cue inspection, and non-blocking readability warnings (CPS > 25, characters > 84).

### 10.2 Subtitle Document & Track Integration
- Applying translation creates a distinct NEW `SubtitleTrack` in `SubtitleDocument.tracks` with language code (e.g. `es`) and display name (e.g. `Spanish (Translated)`).
- Original source tracks and cues remain 100% untouched and immutable.
- Strictly preserves timing: `target_start_us == source_start_us` and `target_end_us == source_end_us`.
- Clears source-language word timestamps (`words: None`) to prevent invalid acoustic alignment assumptions on translated text.
- Translated cues are immediately selectable, editable in the visual timeline and cue inspector, and exportable to `.srt`, `.vtt`, or stylable for video burn-in.

---

## 11. Bilingual Review & Terminology Management Integration (STEP 23)

### 11.1 Dedicated Review & Glossary Workspace Tabs
- Localization Studio includes dedicated **Review** (`BilingualReviewPanel.tsx`) and **Glossary** (`GlossaryManagerPanel.tsx`) tabs.
- Side-by-side view pairs original source subtitles (read-only) with translated target subtitles (editable).
- Selecting any bilingual row automatically seeks video playback and syncs with the editor playhead.
- Direct editing of target text within the bilingual review interface updates canonical `SubtitleDocument` tracks.
- Invalidation safety: Editing approved target text or modifying cue timings immediately resets cue status from `Approved` to `NeedsReview`, clearing the SHA-256 approval fingerprint.

### 11.2 Terminology Protection and Translation Memory
- Glossaries with protected term definitions ensure company, product, or brand names are not mistranslated or altered.
- Real-time diagnostics display findings such as `PROTECTED_TERM_CHANGED`, `EMPTY_TARGET_TEXT`, and `EXCESSIVE_READING_SPEED`.
- Approved translations populate a local Translation Memory (TM) database, surfacing 100% exact match suggestions for repeated phrases with one-click adoption.

---

## 12. Coordination with Dubbing Timeline (STEP 26)

The Subtitle Editor remains the authoritative tool for text caption manipulation while coordinating with the dedicated Dubbing Timeline panel (`DubbingTimelinePanel.tsx`):
- **Coexistence without Replacement:** The Dubbing Timeline is accessible via the "Dubbing" tab in Localization Studio, providing a multi-track audio segment editor without replacing or altering the core Subtitle Editor.
- **Shared Master Playhead:** Seeking in the Subtitle Editor, video player, or Dubbing Timeline stays 100% synchronized through the shared `currentTime` and `seekTo` interfaces.
- **Cue References:** Dubbing segments retain immutable references to their underlying subtitle cues (`source_subtitle_cue_id`, `original_cue_start_us`, `original_cue_end_us`), surfacing overrun diagnostics without altering original cue boundaries.
- **Unified Document Persistence:** Saving changes in either the Subtitle Editor or Dubbing Timeline updates the unified `.omwsub` project document without conflict.


