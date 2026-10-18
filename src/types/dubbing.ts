/**
 * TypeScript Data Contracts for Professional AI Dubbing Timeline.
 * Strict alignment with Rust opentake_core::localization::dubbing::models.
 */

export type DiagnosticSeverity = "info" | "warning" | "error";

export type DubbingDiagnosticCode =
  | "missingAudioSegment"
  | "unresolvedVoiceAssignment"
  | "voiceModelUnavailable"
  | "sourceCueChanged"
  | "segmentOverlap"
  | "segmentOutsideMedia"
  | "segmentExceedsSubtitleInterval"
  | "largeDialogueGap"
  | "invalidSourceTrim"
  | "missingAudioAsset"
  | "corruptedAudioAsset"
  | "languageVoiceMismatch"
  | "duplicateCueSegment";

export interface DubbingDiagnosticFinding {
  code: DubbingDiagnosticCode;
  severity: DiagnosticSeverity;
  trackId?: string;
  segmentId?: string;
  cueId?: string;
  message: string;
  timeRangeUs?: [number, number];
}

export type DubbingSegmentStatus = "ready" | "stale" | "synthesizing" | "missingAsset" | "error";

export interface DubbingAudioAsset {
  assetId: string;
  filePath: string;
  fileName: string;
  sha256: string;
  sampleRate: number;
  channelCount: number;
  durationUs: number;
  sizeBytes: number;
  voiceId: string;
  speakerId?: number;
  createdAtEpochMs: number;
}

export interface DubbingSegment {
  id: string;
  trackId: string;
  sourceSubtitleCueId?: string;
  sourceSubtitleTrackId?: string;
  sourceText: string;
  sourceTextHash: string;
  voiceId: string;
  speakerId?: number;
  audioAssetId: string;
  sourceInUs: number;
  sourceOutUs: number;
  timelineStartUs: number;
  originalCueStartUs?: number;
  originalCueEndUs?: number;
  gain: number;
  status: DubbingSegmentStatus;
  warnings: string[];
  originalAudioAssetId?: string;
  alignedAudioAssetId?: string;
  alignmentMode?: DubbingAlignmentMode;
  alignmentPlan?: DubbingAlignmentPlan;
  alignmentResult?: DubbingAlignmentResult;
  startOffsetUs?: number;
}

export type DubbingAlignmentMode = "original" | "autoFit" | "tempoAdjust" | "padSilence" | "manual";

export type DubbingAlignmentStatus =
  | "notAnalyzed"
  | "ready"
  | "processing"
  | "aligned"
  | "needsReview"
  | "failed"
  | "cancelled";

export type SilencePaddingMode = "noPadding" | "padEnd" | "padStart" | "centered";

export type DubbingAlignmentDiagnosticCode =
  | "speechLongerThanCue"
  | "speechShorterThanCue"
  | "tempoExceedsAutoLimit"
  | "tempoBelowAutoLimit"
  | "alignmentRequiresReview"
  | "alignedDurationMismatch"
  | "dialogueOverlap"
  | "outsideVideoBounds"
  | "missingSourceAudio"
  | "missingAlignedAudio"
  | "corruptedAlignedAudio"
  | "staleAlignment"
  | "sourceTrimChanged"
  | "voiceSegmentRegenerated"
  | "manualOffsetConflict"
  | "insufficientTimelineSpace";

export interface DubbingAlignmentFinding {
  code: DubbingAlignmentDiagnosticCode;
  severity: DiagnosticSeverity;
  segmentId: string;
  message: string;
  durationDiffUs?: number;
}

export interface AlignmentConfig {
  minAutoTempo: number;
  maxAutoTempo: number;
  minSupportedTempo: number;
  maxSupportedTempo: number;
  toleranceUs: number;
  defaultPaddingMode: SilencePaddingMode;
  algorithmVersion: number;
}

export interface DubbingAlignmentPlan {
  segmentId: string;
  originalAssetId: string;
  originalAssetHash: string;
  sourceTrimmedDurationUs: number;
  targetStartUs: number;
  targetEndUs: number;
  targetDurationUs: number;
  requestedStartOffsetUs: number;
  effectiveTargetDurationUs: number;
  proposedTempo: number;
  paddingMode: SilencePaddingMode;
  paddingBeforeUs: number;
  paddingAfterUs: number;
  selectedStrategy: string;
  requiresManualReview: boolean;
  warnings: string[];
  algorithmVersion: number;
}

export interface DubbingAlignmentResult {
  alignmentId: string;
  segmentId: string;
  originalAssetId: string;
  alignedAssetId?: string;
  actualOutputDurationUs: number;
  effectivePlacementStartUs: number;
  effectivePlacementEndUs: number;
  processingMethod: string;
  measuredDurationErrorUs: number;
  tempoFactorApplied: number;
  paddingBeforeAppliedUs: number;
  paddingAfterAppliedUs: number;
  diagnostics: DubbingAlignmentFinding[];
  status: DubbingAlignmentStatus;
  createdAtEpochMs: number;
}

export interface DubbingTrack {
  id: string;
  displayName: string;
  language: string;
  sourceSubtitleTrackId?: string;
  assignedVoiceId?: string;
  assignedSpeakerId?: number;
  segments: DubbingSegment[];
  isMuted: boolean;
  isSolo: boolean;
  isEnabled: boolean;
}

export interface DubbingProject {
  schemaVersion: number;
  id: string;
  sourceProjectId: string;
  mediaReference?: string;
  timelineDurationUs: number;
  sourceLanguage: string;
  targetLanguage: string;
  tracks: DubbingTrack[];
  assets: Record<string, DubbingAudioAsset>;
  mixConfiguration?: DubbingMixConfiguration;
  mixResult?: DubbingMixResult;
  createdAtEpochMs: number;
  updatedAtEpochMs: number;
}

export interface DubbingWaveformBin {
  min: number;
  max: number;
  rms: number;
}

export interface DubbingTimelineWaveform {
  segmentId: string;
  assetId: string;
  sampleRate: number;
  durationUs: number;
  bins: DubbingWaveformBin[];
}

export type MixPreset = "balanced" | "voiceFocus" | "preserveOriginal" | "custom";

export interface DubbingMixTrackSettings {
  trackId: string;
  isEnabled: boolean;
  isMuted: boolean;
  isSolo: boolean;
  gainDb: number;
  pan: number;
}

export interface DubbingMixConfiguration {
  schemaVersion: number;
  projectId: string;
  originalAudioEnabled: boolean;
  originalAudioGainDb: number;
  dubbingMasterGainDb: number;
  trackSettings: Record<string, DubbingMixTrackSettings>;
  duckingEnabled: boolean;
  duckingAttenuationDb: number;
  duckingAttackMs: number;
  duckingReleaseMs: number;
  duckingHoldMs: number;
  fadeInMs: number;
  fadeOutMs: number;
  outputSampleRate: number;
  outputChannels: number;
  preset: MixPreset;
  limiterEnabled: boolean;
  peakCeilingDb: number;
  updatedAtEpochMs: number;
}

export interface DubbingMixSegment {
  segmentId: string;
  trackId: string;
  assetId: string;
  resolvedFilePath: string;
  timelineStartUs: number;
  durationUs: number;
  sourceInUs: number;
  sourceOutUs: number;
  gainDb: number;
  fadeInMs: number;
  fadeOutMs: number;
  isAligned: boolean;
}

export interface DuckingInterval {
  startUs: number;
  endUs: number;
  attackUs: number;
  releaseUs: number;
  attenuationDb: number;
}

export interface DubbingMixPlan {
  planId: string;
  projectId: string;
  timelineDurationUs: number;
  hasOriginalAudio: boolean;
  originalAudioPath?: string;
  originalAudioSampleRate: number;
  originalAudioChannels: number;
  originalGainDb: number;
  dubbingMasterGainDb: number;
  segments: DubbingMixSegment[];
  duckingIntervals: DuckingInterval[];
  outputSampleRate: number;
  outputChannels: number;
  filterGraphDesc: string;
  warnings: string[];
}

export interface DubbingMixResult {
  resultId: string;
  projectId: string;
  mixAssetId: string;
  filePath: string;
  sha256: string;
  actualDurationUs: number;
  sampleRate: number;
  channels: number;
  bitDepth: number;
  fileSizeBytes: number;
  peakDbfs: number;
  rmsDbfs: number;
  integratedLufs?: number;
  clippingSamples: number;
  warnings: string[];
  createdAtEpochMs: number;
}

export interface AudioAnalysisReport {
  sampleRate: number;
  channels: number;
  bitDepth: number;
  durationUs: number;
  peakDbfs: number;
  rmsDbfs: number;
  integratedLufs?: number;
  clippingSamples: number;
}

export interface DubbingMixReadiness {
  isReady: boolean;
  hasOriginalAudio: boolean;
  originalAudioPath?: string;
  totalSpeechSegments: number;
  readySpeechSegments: number;
  missingSpeechSegments: number;
  hasExistingMix: boolean;
  isMixStale: boolean;
  warnings: string[];
}

export interface DubbingMixdownStatus {
  isRendering: boolean;
  progress: number;
  message: string;
  error?: string;
}

export type DubbingOperation =
  | { type: "moveSegment"; segmentId: string; newTimelineStartUs: number }
  | { type: "trimSegmentStart"; segmentId: string; newSourceInUs: number }
  | { type: "trimSegmentEnd"; segmentId: string; newSourceOutUs: number }
  | { type: "splitSegment"; segmentId: string; splitTimelineUs: number }
  | { type: "deleteSegment"; segmentId: string }
  | { type: "duplicateSegment"; segmentId: string; offsetUs: number }
  | { type: "changeTrack"; segmentId: string; targetTrackId: string }
  | { type: "setGain"; segmentId: string; gain: number }
  | { type: "setTrackMute"; trackId: string; isMuted: boolean }
  | { type: "setTrackSolo"; trackId: string; isSolo: boolean }
  | { type: "setTrackEnabled"; trackId: string; isEnabled: boolean }
  | { type: "updateTrackVoice"; trackId: string; voiceId?: string; speakerId?: number }
  | { type: "reorderTracks"; trackIds: string[] }
  | {
      type: "applyAlignment";
      segmentId: string;
      plan: DubbingAlignmentPlan;
      result: DubbingAlignmentResult;
      alignedAsset: DubbingAudioAsset;
    }
  | { type: "restoreOriginalAudio"; segmentId: string }
  | { type: "setAlignmentMode"; segmentId: string; mode: DubbingAlignmentMode }
  | { type: "setStartOffset"; segmentId: string; offsetUs: number }
  | { type: "updateMixConfiguration"; configuration: DubbingMixConfiguration }
  | { type: "applyMixResult"; result: DubbingMixResult }
  | { type: "invalidateMixResult" };
