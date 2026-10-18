# Architecture Document: Professional AI Dubbing Audio Mixing Subsystem
**AI Dubbing Mixdown, Dialogue Ducking, Multitrack Gain Staging, Loudness Analysis & Non-Destructive WAV Rendering**

**Project:** Open Media Workbench  
**Status:** Completed (STEP 28)  
**Workspace:** Localization Studio  
**Offline & Privacy Guarantee:** 100% offline on-device processing via system FFmpeg. Zero cloud APIs, zero telemetry, zero automatic downloads.

---

## 1. Architectural Overview & Design Goals

The Professional Audio Mixing subsystem (`crates/opentake-core/src/localization/dubbing/mixing/`) provides deterministic, sample-accurate, non-destructive multitrack mixdown for video localization projects. It seamlessly bridges:
1. The original authorized video soundtrack (or speech-only mixdown when videos lack audio).
2. Placed AI dubbing dialogue clips (including original Piper TTS WAVs and duration-aligned tempo-stretched/padded WAVs).
3. Configurable dialogue ducking envelopes that automatically attenuate original audio during active speech.
4. Channel and project gain staging (in true decibel units with safe linear conversion).
5. Professional peak limiting (`alimiter` with -1.0 dBFS ceiling) to prevent digital clipping when multiple loud audio streams overlap.
6. Genuine acoustic measurements (Sample Peak dBFS, RMS dBFS, Integrated LUFS, and clipping sample counts).
7. Content-addressed durable mixdown asset caching (`mix_{hash}.wav`) with automatic invalidation upon any edit.
8. Instant A/B preview comparing original video audio against the rendered mixed WAV.

```
┌────────────────────────────────┐     ┌────────────────────────────────┐
│   Original Media Soundtrack    │     │   Durable AI Speech Assets     │
│  (Audio stream or No-Audio)    │     │ (Original TTS / Aligned WAVs)  │
└───────────────┬────────────────┘     └───────────────┬────────────────┘
                │                                      │
                ▼                                      ▼
┌───────────────────────────────────────────────────────────────────────┐
│                     Deterministic Mixdown Planner                     │
│  - Track Solo/Mute Resolution                                        │
│  - Active Segment Asset Resolution (Prevents Double Alignment)        │
│  - Dialogue Ducking Interval Merging (Prevents Volume Pumping)        │
│  - Sample-Accurate Delay Calculation: (timeline_start_us * SR) / 10^6 │
└───────────────────────────────────┬───────────────────────────────────┘
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────┐
│                  System FFmpeg Complex Filtergraph                    │
│  [0:a] aresample=48k, aformat=stereo, volume=orig_gain,               │
│        volume=eval=frame:expr='ducking_envelope' [orig]               │
│  [i:a] aresample=48k, aformat=stereo, volume=gain,                    │
│        afade=in/out, adelay=N|N:all=1 [s_i]                           │
│  [orig][s_0]...[s_k] amix=inputs=K:normalize=0,                       │
│        alimiter=limit=0.891:attack=5:release=50:asc=1 [out]          │
└───────────────────────────────────┬───────────────────────────────────┘
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────┐
│                 Acoustic Analysis & Durable Promotion                 │
│  - PCM 16-bit 48kHz Stereo WAV RIFF Verification                      │
│  - Sample Peak, RMS dBFS, LUFS, Clipping Detection                    │
│  - Atomic Promotion to ~/.local/share/opentake/dubbing/assets/        │
│  - Invalidation of Stale Previews on Any Setting Adjustment          │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 2. Rust Subsystem Modules

The mixing domain is natively implemented in Rust under `crates/opentake-core/src/localization/dubbing/mixing/`:

| Module | Responsibility |
|---|---|
| `models.rs` | Data structures: `DubbingMixConfiguration`, `DubbingMixTrackSettings`, `DubbingMixSegment`, `DuckingInterval`, `DubbingMixPlan`, `DubbingMixResult`, and decibel/linear conversion utilities. |
| `errors.rs` | Strongly-typed `DubbingMixError` taxonomy covering missing assets, format errors, FFmpeg execution failures, and cancellation. |
| `presets.rs` | Standard production mixing presets (`Balanced`, `VoiceFocus`, `PreserveOriginal`, `Custom`). |
| `ducking.rs` | $O(N \log N)$ chronological speech interval merging algorithm with attack pre-roll, hold plateaus, and release decay. |
| `filters.rs` | Complex filtergraph generation: `aresample=48000`, `aformat=stereo:s16`, `adelay`, `afade`, `amix=normalize=0`, and `alimiter`. |
| `planner.rs` | Mixdown planner: probes original audio with FFprobe, resolves track mute/solo rules, and active aligned speech assets. |
| `renderer.rs` | Managed subprocess runner for system FFmpeg, cancellation token support, and disk-full rollback. |
| `analysis.rs` | Grounded acoustic measurement of peak dBFS, RMS dBFS, integrated LUFS, and clipping sample counts. |
| `assets.rs` | Deterministic SHA-256 cache key calculation, durable asset path resolution, and atomic staging promotion. |
| `persistence.rs` | Project operations for persisting mix configurations and results in `DubbingProject` and `SubtitleDocument`. |
| `mod.rs` | Public domain facade. |

---

## 3. Dialogue Ducking Algorithm & Pumping Prevention

To prevent unnatural volume pumping between rapid dialogue utterances, speech intervals separated by less than the envelope threshold:
$$\Delta t_{\text{threshold}} = t_{\text{hold}} + t_{\text{release}} + t_{\text{attack}}$$
are merged into a single continuous ducking span.

For each ducking window, a continuous piecewise envelope is constructed:
- **Attack Phase:** Linear ramp from unity gain (1.0) down to attenuation $A = 10^{\frac{\text{attenuation\_db}}{20}}$ over the configured attack duration ($t_0 \to t_1$).
- **Hold Phase:** Flat plateau held at attenuation $A$ throughout the speech duration and intentional hold padding ($t_1 \to t_2$).
- **Release Phase:** Smooth linear ramp from attenuation $A$ back up to unity gain (1.0) over the configured release duration ($t_2 \to t_3$).
- **Multi-interval Combination:** Piecewise envelopes are combined using `min(e_1, min(e_2, ...))`, guaranteeing smooth behavior with zero discontinuities or clicks.

---

## 4. Timeline Semantics & Double Alignment Prevention

Dubbing segments may either use:
1. Original synthesized Piper TTS speech assets.
2. Duration-aligned assets processed via STEP 27 (`atempo` time-stretching or silence padding).

The mixdown planner inspects `seg.is_aligned()`. When an aligned asset is present:
- The pre-aligned WAV asset is resolved as the input stream.
- No redundant source trim or tempo transformation is re-applied.
- Sample-accurate positioning is achieved using FFmpeg's `adelay=delays={samples}S:all=1`, where:
  $$\text{samples} = \frac{\text{timeline\_start\_us} \times \text{output\_sample\_rate}}{1\,000\,000}$$
- This guarantees absolute sample-frame precision with zero drifting across hours of dialogue.

---

## 5. Peak Protection & Normalization Invariant

FFmpeg's `amix` filter defaults to `normalize=1`, which automatically divides all inputs by the number of active streams ($1/N$). In a dynamic timeline with varying overlapping speakers, this would cause jarring volume drops whenever an additional speaker joins the mix.

Open Media Workbench enforces:
- `amix=inputs=N:normalize=0` to preserve the user's exact decibel gain staging.
- Peak safety is guaranteed downstream by the mastering limiter:
  `alimiter=limit=0.891:attack=5:release=50:asc=1`
  which imposes an unbreakable -1.0 dBFS ceiling.

---

## 6. Tauri 2 IPC Command Endpoints

Registered in `src-tauri/src/lib.rs` and implemented in `src-tauri/src/commands/dubbing.rs`:
- `get_dubbing_mix_configuration`
- `update_dubbing_mix_configuration`
- `get_dubbing_mix_readiness`
- `plan_dubbing_mix`
- `start_dubbing_mixdown`
- `get_dubbing_mixdown_status`
- `cancel_dubbing_mixdown`
- `get_dubbing_mixdown_result`
- `analyze_dubbing_mix_audio`
- `restore_dubbing_mix_configuration`

---

## 7. Quality Gates & Test Coverage

- **Rust Workspace Tests:** 382 total tests passing.
- **Dubbing Mixing Unit Tests (`tests/dubbing_mixing.rs`):** 10 tests passing.
- **Dubbing Mixing E2E Tests (`tests/dubbing_mixing_e2e.rs`):** Real system FFmpeg execution across Cases A through F (original soundtrack + AI speech, dialogue ducking, solo/mute, video without audio, aligned asset integration, peak analysis, and project reload).
- **Frontend Vitest/Node Tests:** 116 tests passing.
- **Clippy:** Zero warnings (`-D warnings`).
- **Formatting:** Clean (`cargo fmt --all -- --check`).
- **Source Media Immutability:** Verified byte-identical SHA-256 before and after rendering.
