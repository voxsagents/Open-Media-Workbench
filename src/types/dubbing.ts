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
  | { type: "reorderTracks"; trackIds: string[] };
