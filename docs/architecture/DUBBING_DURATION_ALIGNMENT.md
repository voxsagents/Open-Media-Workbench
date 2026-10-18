# Architecture: Professional AI Dubbing Duration Alignment

> **Open Media Workbench — STEP 27 Architecture Specification**  
> Status: Implemented & Verified (STEP 27)  
> Workspace: Localization Studio

---

## 1. Executive Summary

The **Professional AI Dubbing Duration Alignment** engine delivers deterministic, pitch-preserving speech time-stretching, sample-accurate silence padding, timeline synchronization optimization, and non-destructive audio cache management into Open Media Workbench's Localization Studio.

Operating 100% offline with system FFmpeg (`atempo` filter) and on-device Piper TTS, this system bridges the acoustic duration gap between generated synthetic speech and target subtitle intervals without altering original voice recordings or subtitle timings.

```mermaid
graph TD
    subgraph UI [Localization Studio: React 19 UI]
        Inspector[DubbingAlignmentInspector]
        Timeline[DubbingTimelinePanel]
        ClipBadge[DubbingSegmentClip Alignment Pill]
        ABPlayer[A/B Preview: Original vs Aligned Audio]
        Controls[Tempo Slider / Silence Pad Selector / Offset Nudge]
    end

    subgraph IPC [Tauri 2 Secure Command Dispatcher]
        PlanCmd[plan_dubbing_alignment]
        ProcessCmd[process_dubbing_alignment]
        ApplyCmd[apply_dubbing_alignment]
        RestoreCmd[restore_original_dubbing_audio]
        BatchCmd[batch_align_dubbing_segments]
        DiagCmd[get_dubbing_alignment_diagnostics]
    end

    subgraph Core [opentake-core :: localization::dubbing::alignment]
        Planner[Deterministic Alignment Planner]
        TempoEngine[FFmpeg atempo Filter Runner]
        PaddingEngine[PCM Sample-Accurate Silence Padder]
        CacheManager[Content-Addressed SHA-256 Storage]
        DiagEngine[Grounded Diagnostics Evaluator]
    end

    subgraph Storage [Durable Asset Vault]
        OrigAsset[Original Piper Speech: ~/.local/share/opentake/dubbing/assets/{id}.wav]
        AlignedAsset[Aligned Speech: ~/.local/share/opentake/dubbing/assets/aligned_{hash}.wav]
    end

    Timeline --> Inspector
    Inspector --> ABPlayer
    Inspector --> Controls
    Inspector --> PlanCmd
    Inspector --> ProcessCmd
    Inspector --> ApplyCmd
    Inspector --> RestoreCmd
    Timeline --> BatchCmd
    Timeline --> DiagCmd

    PlanCmd --> Planner
    ProcessCmd --> TempoEngine
    ProcessCmd --> PaddingEngine
    ProcessCmd --> CacheManager
    ApplyCmd --> Timeline
    DiagCmd --> DiagEngine

    OrigAsset --> TempoEngine
    TempoEngine --> PaddingEngine
    PaddingEngine --> AlignedAsset
```

---

## 2. Canonical Time & Duration Contracts

All timing coordinates and durations strictly use signed or unsigned integer microseconds (`MediaTimeUs`), preventing accumulated rounding errors.

### 2.1 Authoritative Invariant Coordinates

1. **Target Subtitle Window:**  
   $$\Delta T_{\text{target}} = \text{cue\_end\_us} - \text{cue\_start\_us}$$
2. **Trimmed Source Speech Duration:**  
   $$\Delta T_{\text{trimmed}} = \text{source\_out\_us} - \text{source\_in\_us}$$
   *(Never calculated from untrimmed WAV length).*
3. **Effective Target Duration:**  
   $$\Delta T_{\text{effective}} = \Delta T_{\text{target}} - \max(0, \text{start\_offset\_us})$$
4. **Calculated Overrun / Underrun:**  
   $$\text{diff\_us} = \Delta T_{\text{trimmed}} - \Delta T_{\text{effective}}$$
5. **Required Tempo Speedup Factor:**  
   $$F_{\text{tempo}} = \frac{\Delta T_{\text{trimmed}}}{\Delta T_{\text{effective}}}$$

---

## 3. Deterministic Alignment Planner

The planner (`planner.rs`) acts as an explainable decision system:

| Case | Condition | Strategy | Output Status |
| :--- | :--- | :--- | :--- |
| **Exact Fit** | $|\text{diff\_us}| \le 25,000\,\mu\text{s}$ (25ms) | Preserve original tempo ($1.0\times$), No padding | `Ready` |
| **Short Speech** | $\text{diff\_us} < -25,000\,\mu\text{s}$ | Keep $1.0\times$ tempo, Pad with silence (`PadEnd`, `PadStart`, or `Centered`) | `Ready` |
| **Moderate Overrun** | $0 < \text{diff\_us}$, $1.0 < F_{\text{tempo}} \le 1.15$ | Accelerate with pitch-preserving `atempo`, no padding | `Ready` |
| **Configured Overrun** | $1.15 < F_{\text{tempo}} \le 1.25$ | Safe upper bound within user policy ($1.25\times$) | `Ready` |
| **Large Overrun** | $F_{\text{tempo}} > \text{max\_tempo}$ ($>1.25\times$) | Flag for manual editor review; clamp tempo or request script trim | `NeedsReview` |
| **Extreme Overrun** | $F_{\text{tempo}} > 2.0\times$ | Reject automatic fitting; preserve speech naturalness | `NeedsReview` |

---

## 4. Pitch-Preserving Tempo Engine (`atempo`)

- **Tool:** Verified system FFmpeg binary executed via `run_managed_process`.
- **Filter Syntax:** `-filter:a atempo=<factor>`.
- **Filter Chaining:** For factors outside $[0.5, 2.0]$, the engine deterministically chains multiple atempo passes (e.g. `atempo=2.0,atempo=1.2` for $2.4\times$).
- **Structured Invocation:** Shell-less process arguments prevent command injection.
- **Verification:** The output WAV header is parsed and actual sample frames are counted:
  $$\text{Measured Duration} = \frac{\text{sample\_frames} \times 1,000,000}{\text{sample\_rate}}$$

---

## 5. Sample-Accurate Silence Padding

- Silence padding is performed directly in 16-bit PCM sample buffers (`padding.rs`), avoiding FFmpeg filter latency and rounding discrepancies.
- **Sample Count Calculation:**
  $$N_{\text{samples}} = \left\lfloor \frac{\Delta T_{\text{pad\_us}} \times \text{sample\_rate}}{1,000,000} \right\rfloor \times \text{channels}$$
- Supported Modes:
  - `PadEnd` (default): Speech starts immediately with video cue; trailing silence preserves dialogue rhythm.
  - `PadStart`: Speech is delayed to match late subtitle cadence.
  - `Centered`: Equal silence distribution before and after speech.
  - `NoPadding`: Output duration matches trimmed audio duration exactly.

---

## 6. Durable Asset Management & Content-Addressed Caching

Aligned audio is never written into the source speech WAV. Instead, an atomic promotion workflow is enforced:

1. **Staging:** Output is rendered into a temporary unique staging path.
2. **Validation:** Sample rate (22,050 Hz), mono channel count, and bit depth (16-bit) are verified.
3. **Cache Key Computation:**
   $$\text{Key} = \text{SHA256}(\text{source\_hash} + \text{source\_in} + \text{source\_out} + \text{tempo} + \text{pad\_mode} + \text{offsets} + \text{algo\_version})$$
4. **Promotion:** Staged file is moved atomically to `~/.local/share/opentake/dubbing/assets/aligned_{cache_key}.wav`.
5. **Timeline Update:** Segment's `aligned_audio_asset_id` is updated, while `original_audio_asset_id` remains intact.
6. **Restoration:** Users can switch back to `original` audio at any time with zero re-rendering.

---

## 7. A/B Audio Audition & Video Playhead Synchronization

- **True A/B Preview:** `DubbingAlignmentInspector` mounts two distinct HTML5 audio streams using the secure `opentake-media://` protocol.
- **Track Selection:** The user can toggle between Original TTS speech and Processed Aligned speech.
- **Playhead Lock:** Playback offsets match the authoritative timeline video clock.
- **Conflict Diagnostics:** Dialogue collisions and media boundary violations are highlighted in real-time.

---

## 8. Undo/Redo & Project Persistence

- Operations (`ApplyAlignment`, `RestoreOriginalAudio`, `SetAlignmentMode`, `SetStartOffset`) integrate seamlessly into the 50-step bounded history.
- Project metadata serializes alignment plans, results, and active asset IDs directly into `SubtitleDocument` metadata (`.omwsub`), guaranteeing offline reopen compatibility.
