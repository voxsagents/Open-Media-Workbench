# Open Media Workbench — Licensing & Distribution Audit

**Document Version:** 1.0.0  
**Target Release:** v0.1.0 Beta  
**Status:** Canonical Licensing Architecture & Compliance Baseline  
**Date:** 2026-10-14  

---

## 1. Executive Summary & Audit Scope

Open Media Workbench (formerly OpenTake) is an offline-first desktop application developed using Tauri 2, Rust, and React 19. It provides an extensible media engine hosting two future specialized workspaces:
- **Repair Studio** (media corruption diagnosis, atom reconstruction, stream salvage)
- **Localization Studio** (subtitles, offline speech-to-text, translation, neural voice dubbing)

This audit establishes the legal and licensing framework for the project, analyzing:
1. Current repository licensing status.
2. FFmpeg / FFprobe runtime integration vs distribution obligations.
3. Existing Rust and JavaScript crate/package dependencies.
4. Future candidate technologies: Untrunc, local Whisper, neural machine translation, and TTS models.
5. Distribution obligations and supply-chain risk mitigation.

---

## 2. Confirmed License Baseline (Active Repository)

### 2.1 Project License
- **Status:** **Confirmed (Declared in Workspace Cargo Manifests)**
- **License Expression:** `GPL-3.0-or-later` (GNU General Public License v3.0 or later)
- **Declaration Sites:**
  - `crates/opentake-core/Cargo.toml` (`license = "GPL-3.0-or-later"`)
  - `src-tauri/Cargo.toml` (`license = "GPL-3.0-or-later"`)
  - Architectural references in `docs/licensing/OPEN_SOURCE_LICENSE.md` and `docs/decisions/ADR-0001.md`.
- **License Invariant:** The core application license has **not been altered** during rebranding.

### 2.2 Active Rust Workspace Dependencies (`Cargo.lock`)

All current Rust dependencies utilize permissive licenses (MIT, Apache-2.0, or dual MIT/Apache-2.0), making them 100% compatible for incorporation into a GPL-3.0-or-later binary:

| Crate | Version | License | Category / Purpose |
| :--- | :--- | :--- | :--- |
| `tokio` / `tokio-util` | 1.43 / 0.7 | MIT | Async runtime & process IO |
| `serde` / `serde_json` | 1.0 / 1.0 | MIT / Apache-2.0 | Serialization & IPC typing |
| `thiserror` | 2.0 | MIT / Apache-2.0 | Typed error derivation |
| `uuid` | 1.15 | MIT / Apache-2.0 | Media asset identifiers |
| `sha2` | 0.10 | MIT / Apache-2.0 | Media fingerprinting & cache keys |
| `libc` | 0.2 | MIT / Apache-2.0 | Process group isolation (POSIX) |
| `rfd` | 0.15 | MIT | Native file dialog integration |
| `tauri` / `tauri-build` | 2.x | Apache-2.0 / MIT | Desktop windowing and IPC bridge |
| `tempfile` *(dev)* | 3.x | MIT / Apache-2.0 | Test isolation |
| `proptest` *(dev)* | 1.x | MIT / Apache-2.0 | Property testing |

### 2.3 Active JavaScript / Web Dependencies (`package.json`)

All active frontend packages are permissively licensed (MIT or Apache-2.0):

| Package | Version | License | Purpose |
| :--- | :--- | :--- | :--- |
| `react` / `react-dom` | 19.3.0 | MIT | UI rendering engine |
| `@tauri-apps/api` | 2.12.3 | Apache-2.0 / MIT | Tauri IPC client |
| `@tauri-apps/cli` | 2.12.2 | Apache-2.0 / MIT | Build tooling |
| `tailwindcss` / `@tailwindcss/vite` | 4.0.0 | MIT | Design system |
| `vite` / `@vitejs/plugin-react` | 6.2.0 / 4.3.4 | MIT | Bundler and dev server |
| `typescript` | 5.7.2 | Apache-2.0 | Static typechecker |
| `tsx` | 4.23.15 | MIT | Fast TypeScript test runner |
| `eslint` | 9.20.0 | MIT | Code quality verification |

---

## 3. FFmpeg & FFprobe Licensing & Distribution Architecture

### 3.1 Host Binary Configuration Reality
During runtime, the application probes local binaries. On developer and modern Linux desktop hosts, FFmpeg is typically compiled with:
```text
--enable-gpl --enable-libx264 --enable-libx265 ...
```
Because `--enable-gpl` and `--enable-libx264` are activated, the effective license of such FFmpeg binaries is **GPLv2 or GPLv3**.

### 3.2 Process Boundary Isolation
- **Architectural Implementation:** The application communicates with FFmpeg and FFprobe purely across an operating system process boundary (`tokio::process::Command` with standard streams: stdin/stdout/stderr).
- **Address Space Separation:** Open Media Workbench does not link against `libavcodec`, `libavformat`, or `libavutil` in-process.
- **Copyleft Boundary:** Independent process execution allows modular separation; however, **redistributing** the binary triggers distribution obligations.

### 3.3 Distribution Strategies Compared
1. **System Discovery (Current Default — Zero Bundling):**
   - The application discovers user-installed `ffmpeg` and `ffprobe` via `$PATH` and security-audited standard locations (`/usr/bin`, `/usr/local/bin`).
   - **Distribution Obligation:** None. The application redistributes no third-party media binaries.
2. **Bundled Sidecars (Future Installer Packaging):**
   - If static FFmpeg binaries are bundled inside `.AppImage`, `.deb`, `.dmg`, or `.exe` installers, GPL-3.0 source distribution obligations apply: the corresponding source code, build scripts, and configuration flags for that exact binary must be made available to recipients.

---

## 4. Planned Future Technologies Audit (Repair & Localization)

*The following technologies are planned for future integration. None have been added to the codebase in STEP 10.*

### 4.1 Untrunc (Repair Studio Candidate)
- **Origin / Purpose:** Recovery of truncated or unfinalized MP4/MOV videos missing `moov` atom by reconstructing sample tables from a working reference video.
- **License:** **GPL-2.0-or-later**.
- **Compatibility Analysis:**
  - GPL-2.0-or-later is legally compatible with Open Media Workbench's GPL-3.0-or-later license.
  - If integrated directly as C++/Rust source, the resulting composite work is licensed under GPL-3.0.
  - If executed as an external managed subprocess, process boundary rules apply.
- **Verification Item:** Re-implementing atom repair natively in pure safe Rust (`crates/opentake-core::repair`) is architecturally preferable to wrapping external C++ binaries, eliminating external compilation overhead and memory-safety risks.

### 4.2 Local AI Transcription: Whisper Engine (Localization Studio Candidate)
- **Engine Candidates:**
  - `whisper.cpp` (C/C++ port with Rust bindings, e.g., `whisper-rs`).
  - Pure Rust implementations (`candle-transformers` or `tract`).
- **Engine Code License:** MIT License (confirmed for both `whisper.cpp` and Hugging Face Candle).
- **Model Weights Licensing (Critical Distinction):**
  - OpenAI Whisper model weights (`tiny`, `base`, `small`, `medium`) are released under the **MIT License**.
  - **Distribution Strategy:** Model weights must not be committed to Git. Instead, they must be downloaded on-demand with explicit user consent or supplied locally by the user.

### 4.3 Local AI Machine Translation (STEP 22 Implemented)
- **Selected Engine Backend:** `llama.cpp` CLI binary (MIT License).
  - Executed via secure, managed subprocess isolation without shell invocation.
  - License compatibility: MIT License is 100% compatible with GPL-3.0-or-later composite works.
- **Model Weights Licensing:**
  - `qwen2.5-0.5b-instruct` (GGUF Q4_K_M): Apache-2.0 License.
  - Verified and cryptographically pinned. Permissive commercial-friendly license.
  - Distribution Strategy: Zero bundled weights; explicit user download or manual placement.

### 4.4 Local Neural Text-to-Speech (TTS Engine) (STEP 24 Implemented)
- **Engine Runtime:** Piper TTS (`v2023.11.14-2`, CLI `1.2.0`).
- **Engine Code License:** MIT License.
- **Runtime Dependencies:**
  - `libonnxruntime.so.1.14.1`: MIT License.
  - `libpiper_phonemize.so.1.2.0`: MIT License.
  - `libespeak-ng.so.1.52.0.1`: GPL-3.0-or-later.
  - Phoneme dictionaries: GPL-3.0-or-later.
- **Voice Model License:**
  - `piper-en-lessac-medium`: Public Domain (Donald Derrick recording, CC0/Public Domain).
  - Verified and cryptographically pinned (`5efe09e6...` for `.onnx`, `efe19c41...` for `.onnx.json`).
- **Compatibility:** Runtime dependencies and voice model terms are 100% compliant with the Open Media Workbench GPL-3.0-or-later distribution license. Non-commercial (CC-BY-NC) voices remain strictly filtered out from the default catalog.

---

## 5. Compliance Matrix & Verification Status

| Component / Layer | Current Status | Confirmed License | Action Required / Policy |
| :--- | :--- | :--- | :--- |
| **Workbench Rust Core** | Integrated (Steps 1–9) | `GPL-3.0-or-later` | Maintain existing headers and manifests |
| **Desktop Shell (Tauri 2)** | Integrated (Steps 1–9) | `GPL-3.0-or-later` | Maintain existing headers and manifests |
| **Rust Dependencies** | Integrated (Steps 1–9) | MIT / Apache-2.0 | Maintain lockfile integrity |
| **Frontend Dependencies** | Integrated (Steps 1–9) | MIT / Apache-2.0 | Maintain lockfile integrity |
| **FFmpeg / FFprobe** | External Subprocess | GPLv2+ / GPLv3 (Host) | Rely on system path; defer bundling to packaging step |
| **Untrunc / Atom Reconstructor** | Integrated (Step 14) | Safe Rust Native (`GPL-3.0+`) / Untrunc Subprocess (`GPL-2.0+`) | Primary native safe Rust implementation in core; optional external CLI adapter |
| **Shared AI Model Manager** | Integrated (Step 18) | GPL-3.0-or-later (Rust Core) | Zero bundled weights; explicit consent & SHA-256 verification |
| **Whisper Models (Tiny/Base)** | Verified Catalog (Step 18) | MIT License (Weights & Code) | Explicit opt-in download; zero bundling |
| **Piper TTS Models (Lessac)** | Integrated (Step 24/25) | Public Domain (Dataset & Weights) | Strict voice-model license filter allowlist |
| **Piper TTS Models (LibriTTS)** | Verified Catalog (Step 25) | CC-BY-4.0 | Requires attribution notice |
| **Piper TTS Models (Sharvard)** | Verified Catalog (Step 25) | Open Software License / Permissive | Verified permissive license |
| **Piper TTS Models (Ibrahim)** | Verified Catalog (Step 25) | Open Software License / Permissive | Verified permissive license |
| **Qwen 2.5 Text Models** | Verified Catalog (Step 18/22) | Apache-2.0 License (Weights) | Explicit opt-in download; zero bundling |
| **Translation Engine (llama.cpp)** | Integrated (Step 22) | MIT License (Runtime CLI) | Local execution; zero cloud APIs |
| **Translation Engine (Opus-MT)** | Catalog-Only (Step 18) | CC-BY-4.0 | Installation disabled pending Marian runtime integration |
| **Dubbing Duration Alignment (FFmpeg atempo)** | Integrated (Step 27) | System FFmpeg Subprocess / Safe Rust Native | No bundled GPL binaries; pure system atempo filter & native PCM padding |

---

## 6. Conclusion & Next Steps

1. **No Third-Party Binaries Added:** STEP 10 introduces zero third-party binaries, sidecars, or model weights.
2. **License Invariance:** The project's proposed GPL-3.0-or-later status remains completely intact and verified.
3. **Distribution Readiness:** System-discovery architecture ensures clean development without premature redistribution burdens.
