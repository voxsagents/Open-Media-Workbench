/**
 * Client IPC bindings and local state service for Professional AI Dubbing Timeline.
 */

import { invoke } from '@tauri-apps/api/core';
import type { SubtitleDocument } from '../types/subtitles';
import type { TtsVoiceSettings } from '../types/tts';
import type {
  AlignmentConfig,
  AudioAnalysisReport,
  DubbingAlignmentFinding,
  DubbingAlignmentMode,
  DubbingAlignmentPlan,
  DubbingAlignmentResult,
  DubbingAudioAsset,
  DubbingDiagnosticFinding,
  DubbingMixConfiguration,
  DubbingMixdownStatus,
  DubbingMixPlan,
  DubbingMixReadiness,
  DubbingMixResult,
  DubbingOperation,
  DubbingProject,
  DubbingTimelineWaveform,
  MixPreset,
  SilencePaddingMode,
} from '../types/dubbing';

function isTauriEnvironment(): boolean {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  );
}

// In-memory mock state for web/test environment
let mockActiveProject: DubbingProject | null = null;
const mockHistory: DubbingProject[] = [];
let mockRedoStack: DubbingProject[] = [];

/**
 * Creates a new dubbing project from a subtitle track.
 */
export async function createDubbingProject(
  document: SubtitleDocument,
  sourceSubtitleTrackId: string,
  targetLanguage?: string
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('create_dubbing_project', {
      document,
      sourceSubtitleTrackId,
      targetLanguage,
    });
  }

  const subTrack = document.tracks.find((t) => t.id === sourceSubtitleTrackId);
  const lang = targetLanguage || subTrack?.language || 'en';
  const trackId = `track-mock-${Date.now()}`;

  mockActiveProject = {
    schemaVersion: 1,
    id: `dub-proj-${Date.now()}`,
    sourceProjectId: document.id,
    mediaReference: document.associatedMediaAssetId,
    timelineDurationUs: subTrack ? Math.max(...subTrack.cues.map((c) => c.endUs), 10_000_000) : 10_000_000,
    sourceLanguage: subTrack?.language || 'en',
    targetLanguage: lang,
    tracks: [
      {
        id: trackId,
        displayName: `${subTrack?.displayName || 'Dialogue'} Dubbing`,
        language: lang,
        sourceSubtitleTrackId,
        assignedVoiceId: 'piper-en-lessac-medium',
        segments: [],
        isMuted: false,
        isSolo: false,
        isEnabled: true,
      },
    ],
    assets: {},
    createdAtEpochMs: Date.now(),
    updatedAtEpochMs: Date.now(),
  };

  mockHistory.length = 0;
  mockRedoStack.length = 0;
  return mockActiveProject;
}

/**
 * Retrieves the currently active dubbing project from backend memory.
 */
export async function getActiveDubbingProject(): Promise<DubbingProject | null> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject | null>('get_active_dubbing_project');
  }
  return mockActiveProject;
}

/**
 * Sets or updates the active dubbing project.
 */
export async function setActiveDubbingProject(project: DubbingProject): Promise<void> {
  if (isTauriEnvironment()) {
    await invoke('set_active_dubbing_project', { project });
    return;
  }
  mockActiveProject = project;
}

/**
 * Loads an embedded dubbing project from SubtitleDocument metadata.
 */
export async function loadDubbingFromDocument(
  document: SubtitleDocument
): Promise<DubbingProject | null> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject | null>('load_dubbing_from_document', { document });
  }

  if (document.metadata && document.metadata['dubbingProject']) {
    try {
      mockActiveProject = JSON.parse(document.metadata['dubbingProject']) as DubbingProject;
      return mockActiveProject;
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Embeds the active dubbing project into a SubtitleDocument metadata and returns it.
 */
export async function saveDubbingToDocument(
  document: SubtitleDocument
): Promise<SubtitleDocument> {
  if (isTauriEnvironment()) {
    return await invoke<SubtitleDocument>('save_dubbing_to_document', { document });
  }

  const updatedDoc = { ...document, metadata: { ...document.metadata } };
  if (mockActiveProject) {
    updatedDoc.metadata['dubbingProject'] = JSON.stringify(mockActiveProject);
  }
  return updatedDoc;
}

/**
 * Saves the active dubbing project to a standalone file.
 */
export async function saveDubbingToFile(filePath: string): Promise<void> {
  if (isTauriEnvironment()) {
    await invoke('save_dubbing_to_file', { filePath });
  }
}

/**
 * Loads a dubbing project from a standalone file.
 */
export async function loadDubbingFromFile(filePath: string): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('load_dubbing_from_file', { filePath });
  }
  if (!mockActiveProject) {
    throw new Error('No mock project available');
  }
  return mockActiveProject;
}

/**
 * Synthesizes offline speech for cues and places them on the timeline.
 */
export async function generateDubbingSegments(
  dubbingTrackId: string,
  document: SubtitleDocument,
  cueIds?: string[],
  voiceOverride?: string,
  speakerOverride?: number,
  settings?: TtsVoiceSettings
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('generate_dubbing_segments', {
      dubbingTrackId,
      document,
      cueIds,
      voiceOverride,
      speakerOverride,
      settings,
    });
  }

  if (!mockActiveProject) {
    throw new Error('No active dubbing project found');
  }

  // Record undo state
  mockHistory.push(JSON.parse(JSON.stringify(mockActiveProject)));
  mockRedoStack = [];

  const track = mockActiveProject.tracks.find((t) => t.id === dubbingTrackId);
  if (!track) throw new Error(`Track ${dubbingTrackId} not found`);

  const subTrack = document.tracks.find((t) => t.id === track.sourceSubtitleTrackId);
  const cues = subTrack
    ? subTrack.cues.filter((c) => !cueIds || cueIds.includes(c.id))
    : [];

  const voiceId = voiceOverride || track.assignedVoiceId || 'piper-en-lessac-medium';

  for (const cue of cues) {
    if (!cue.text.trim()) continue;
    const assetId = `mock-asset-${cue.id}-${Date.now()}`;
    const cueDurationUs = cue.endUs - cue.startUs;
    const audioDurationUs = Math.max(cueDurationUs, 1_800_000);

    mockActiveProject.assets[assetId] = {
      assetId,
      filePath: `/mock/path/${assetId}.wav`,
      fileName: `${assetId}.wav`,
      sha256: 'mock-hash',
      sampleRate: 22050,
      channelCount: 1,
      durationUs: audioDurationUs,
      sizeBytes: Math.round((audioDurationUs / 1_000_000) * 44100),
      voiceId,
      speakerId: speakerOverride,
      createdAtEpochMs: Date.now(),
    };

    const segId = `seg-${cue.id}`;
    const newSeg = {
      id: segId,
      trackId: dubbingTrackId,
      sourceSubtitleCueId: cue.id,
      sourceText: cue.text,
      sourceTextHash: 'mock-hash',
      voiceId,
      speakerId: speakerOverride,
      audioAssetId: assetId,
      sourceInUs: 0,
      sourceOutUs: audioDurationUs,
      timelineStartUs: cue.startUs,
      originalCueStartUs: cue.startUs,
      originalCueEndUs: cue.endUs,
      gain: 1.0,
      status: 'ready' as const,
      warnings: [],
    };

    const existingIdx = track.segments.findIndex((s) => s.sourceSubtitleCueId === cue.id);
    if (existingIdx >= 0) {
      track.segments[existingIdx] = newSeg;
    } else {
      track.segments.push(newSeg);
    }
  }

  track.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
  mockActiveProject.updatedAtEpochMs = Date.now();
  return { ...mockActiveProject };
}

/**
 * Regenerates a single segment.
 */
export async function regenerateDubbingSegment(
  segmentId: string,
  newText?: string,
  voiceOverride?: string,
  speakerOverride?: number,
  settings?: TtsVoiceSettings
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('regenerate_dubbing_segment', {
      segmentId,
      newText,
      voiceOverride,
      speakerOverride,
      settings,
    });
  }

  if (!mockActiveProject) throw new Error('No active project');
  mockHistory.push(JSON.parse(JSON.stringify(mockActiveProject)));
  mockRedoStack = [];

  for (const track of mockActiveProject.tracks) {
    const seg = track.segments.find((s) => s.id === segmentId);
    if (seg) {
      if (newText) seg.sourceText = newText;
      if (voiceOverride) seg.voiceId = voiceOverride;
      if (speakerOverride !== undefined) seg.speakerId = speakerOverride;
      const assetId = `mock-regen-${Date.now()}`;
      mockActiveProject.assets[assetId] = {
        assetId,
        filePath: `/mock/path/${assetId}.wav`,
        fileName: `${assetId}.wav`,
        sha256: 'mock-regen-hash',
        sampleRate: 22050,
        channelCount: 1,
        durationUs: seg.sourceOutUs,
        sizeBytes: 88200,
        voiceId: seg.voiceId,
        speakerId: seg.speakerId,
        createdAtEpochMs: Date.now(),
      };
      seg.audioAssetId = assetId;
      break;
    }
  }
  return { ...mockActiveProject };
}

/**
 * Applies a non-destructive editing operation.
 */
export async function applyDubbingOperation(
  operation: DubbingOperation
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('apply_dubbing_operation', { operation });
  }

  if (!mockActiveProject) throw new Error('No active project');
  mockHistory.push(JSON.parse(JSON.stringify(mockActiveProject)));
  mockRedoStack = [];

  switch (operation.type) {
    case 'moveSegment': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          seg.timelineStartUs = Math.max(0, operation.newTimelineStartUs);
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'trimSegmentStart': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg && operation.newSourceInUs < seg.sourceOutUs) {
          const delta = operation.newSourceInUs - seg.sourceInUs;
          seg.sourceInUs = operation.newSourceInUs;
          seg.timelineStartUs += delta;
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'trimSegmentEnd': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg && operation.newSourceOutUs > seg.sourceInUs) {
          seg.sourceOutUs = operation.newSourceOutUs;
          break;
        }
      }
      break;
    }
    case 'splitSegment': {
      for (const t of mockActiveProject.tracks) {
        const idx = t.segments.findIndex((s) => s.id === operation.segmentId);
        if (idx >= 0) {
          const orig = t.segments[idx];
          const splitOffset = operation.splitTimelineUs - orig.timelineStartUs;
          const origDur = orig.sourceOutUs - orig.sourceInUs;
          if (splitOffset > 0 && splitOffset < origDur) {
            const splitSourcePoint = orig.sourceInUs + splitOffset;
            const leftSeg = {
              ...orig,
              id: `${orig.id}-left`,
              sourceOutUs: splitSourcePoint,
            };
            const rightSeg = {
              ...orig,
              id: `${orig.id}-right`,
              sourceInUs: splitSourcePoint,
              timelineStartUs: operation.splitTimelineUs,
            };
            t.segments.splice(idx, 1, leftSeg, rightSeg);
          }
          break;
        }
      }
      break;
    }
    case 'deleteSegment': {
      for (const t of mockActiveProject.tracks) {
        t.segments = t.segments.filter((s) => s.id !== operation.segmentId);
      }
      break;
    }
    case 'duplicateSegment': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          const dup = {
            ...seg,
            id: `seg-dup-${Date.now()}`,
            timelineStartUs: seg.timelineStartUs + operation.offsetUs,
          };
          t.segments.push(dup);
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'setGain': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          seg.gain = Math.max(0, Math.min(2, operation.gain));
          break;
        }
      }
      break;
    }
    case 'setTrackMute': {
      const t = mockActiveProject.tracks.find((x) => x.id === operation.trackId);
      if (t) t.isMuted = operation.isMuted;
      break;
    }
    case 'setTrackSolo': {
      const t = mockActiveProject.tracks.find((x) => x.id === operation.trackId);
      if (t) t.isSolo = operation.isSolo;
      break;
    }
    case 'setTrackEnabled': {
      const t = mockActiveProject.tracks.find((x) => x.id === operation.trackId);
      if (t) t.isEnabled = operation.isEnabled;
      break;
    }
    case 'changeTrack': {
      let movedSeg: DubbingProject['tracks'][0]['segments'][0] | null = null;
      for (const t of mockActiveProject.tracks) {
        const idx = t.segments.findIndex((s) => s.id === operation.segmentId);
        if (idx >= 0) {
          movedSeg = t.segments.splice(idx, 1)[0];
          break;
        }
      }
      if (movedSeg) {
        const target = mockActiveProject.tracks.find((t) => t.id === operation.targetTrackId);
        if (target) {
          movedSeg.trackId = target.id;
          target.segments.push(movedSeg);
          target.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
        }
      }
      break;
    }
    case 'applyAlignment': {
      mockActiveProject.assets[operation.alignedAsset.assetId] = operation.alignedAsset;
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          if (!seg.originalAudioAssetId) {
            seg.originalAudioAssetId = seg.audioAssetId;
          }
          seg.alignedAudioAssetId = operation.alignedAsset.assetId;
          seg.audioAssetId = operation.alignedAsset.assetId;
          seg.alignmentPlan = operation.plan;
          seg.alignmentResult = operation.result;
          seg.alignmentMode = 'autoFit';
          seg.startOffsetUs = operation.plan.requestedStartOffsetUs;
          seg.sourceInUs = 0;
          seg.sourceOutUs = operation.result.actualOutputDurationUs;
          if (operation.plan.requestedStartOffsetUs !== 0) {
            seg.timelineStartUs = operation.plan.targetStartUs + Math.max(0, operation.plan.requestedStartOffsetUs);
          }
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'restoreOriginalAudio': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg && seg.originalAudioAssetId) {
          seg.audioAssetId = seg.originalAudioAssetId;
          seg.alignmentMode = 'original';
          const origAsset = mockActiveProject.assets[seg.originalAudioAssetId];
          if (origAsset) {
            seg.sourceInUs = 0;
            seg.sourceOutUs = origAsset.durationUs;
          }
          if (seg.originalCueStartUs !== undefined) {
            seg.timelineStartUs = seg.originalCueStartUs;
          }
          seg.startOffsetUs = 0;
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'setAlignmentMode': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          seg.alignmentMode = operation.mode;
          if (operation.mode === 'original') {
            seg.audioAssetId = seg.originalAudioAssetId || seg.audioAssetId;
          } else if (seg.alignedAudioAssetId) {
            seg.audioAssetId = seg.alignedAudioAssetId;
          }
          const activeAsset = mockActiveProject.assets[seg.audioAssetId];
          if (activeAsset) {
            seg.sourceInUs = 0;
            seg.sourceOutUs = activeAsset.durationUs;
          }
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    case 'setStartOffset': {
      for (const t of mockActiveProject.tracks) {
        const seg = t.segments.find((s) => s.id === operation.segmentId);
        if (seg) {
          seg.startOffsetUs = operation.offsetUs;
          const baseStart = seg.originalCueStartUs ?? seg.timelineStartUs;
          seg.timelineStartUs = operation.offsetUs >= 0
            ? baseStart + operation.offsetUs
            : Math.max(0, baseStart + operation.offsetUs);
          t.segments.sort((a, b) => a.timelineStartUs - b.timelineStartUs);
          break;
        }
      }
      break;
    }
    default:
      break;
  }

  mockActiveProject.updatedAtEpochMs = Date.now();
  return { ...mockActiveProject };
}

/**
 * Undoes the last operation.
 */
export async function undoDubbingOperation(): Promise<DubbingProject | null> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject | null>('undo_dubbing_operation');
  }

  if (mockHistory.length > 0 && mockActiveProject) {
    mockRedoStack.push(mockActiveProject);
    mockActiveProject = mockHistory.pop() || null;
    return mockActiveProject;
  }
  return null;
}

/**
 * Redoes the next undone operation.
 */
export async function redoDubbingOperation(): Promise<DubbingProject | null> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject | null>('redo_dubbing_operation');
  }

  if (mockRedoStack.length > 0 && mockActiveProject) {
    mockHistory.push(mockActiveProject);
    mockActiveProject = mockRedoStack.pop() || null;
    return mockActiveProject;
  }
  return null;
}

/**
 * Loads waveform peak summary for a segment.
 */
export async function getDubbingWaveform(
  segmentId: string
): Promise<DubbingTimelineWaveform> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingTimelineWaveform>('get_dubbing_waveform', { segmentId });
  }

  // Generate bounded synthetic waveform for browser mock preview
  const binsCount = 60;
  const bins = Array.from({ length: binsCount }, (_, i) => {
    const val = Math.sin(i * 0.3) * 0.7;
    return {
      min: -Math.abs(val) * 0.8,
      max: Math.abs(val),
      rms: Math.abs(val) * 0.5,
    };
  });

  return {
    segmentId,
    assetId: 'mock-asset',
    sampleRate: 22050,
    durationUs: 2_000_000,
    bins,
  };
}

/**
 * Retrieves timeline diagnostic findings.
 */
export async function getDubbingDiagnostics(
  document?: SubtitleDocument
): Promise<DubbingDiagnosticFinding[]> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingDiagnosticFinding[]>('get_dubbing_diagnostics', { document });
  }

  if (!mockActiveProject) return [];
  const findings: DubbingDiagnosticFinding[] = [];

  for (const track of mockActiveProject.tracks) {
    for (let i = 0; i < track.segments.length; i++) {
      const seg = track.segments[i];
      const dur = seg.sourceOutUs - seg.sourceInUs;
      const endUs = seg.timelineStartUs + dur;

      // Overrun check
      if (seg.originalCueEndUs && endUs > seg.originalCueEndUs) {
        const overrunMs = Math.round((endUs - seg.originalCueEndUs) / 1000);
        findings.push({
          code: 'segmentExceedsSubtitleInterval',
          severity: 'warning',
          trackId: track.id,
          segmentId: seg.id,
          message: `Speech duration overruns subtitle interval by +${overrunMs}ms`,
        });
      }

      // Overlap check
      if (i > 0) {
        const prev = track.segments[i - 1];
        const prevEnd = prev.timelineStartUs + (prev.sourceOutUs - prev.sourceInUs);
        if (seg.timelineStartUs < prevEnd) {
          findings.push({
            code: 'segmentOverlap',
            severity: 'error',
            trackId: track.id,
            segmentId: seg.id,
            message: `Speech segment collides with previous segment on track "${track.displayName}"`,
          });
        }
      }
    }
  }

  return findings;
}

/**
 * Resolves safe playback URL for an asset.
 */
export async function getDubbingAssetUrl(assetId: string): Promise<string> {
  if (isTauriEnvironment()) {
    return await invoke<string>('get_dubbing_asset_url', { assetId });
  }
  return `opentake-media://localhost/${assetId}`;
}

/**
 * Plans duration alignment for a dubbing segment.
 */
export async function planDubbingAlignment(
  segmentId: string,
  config?: AlignmentConfig,
  manualTempo?: number,
  manualPadding?: SilencePaddingMode,
  manualOffsetUs?: number
): Promise<DubbingAlignmentPlan> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingAlignmentPlan>('plan_dubbing_alignment', {
      segmentId,
      config,
      manualTempo,
      manualPadding,
      manualOffsetUs,
    });
  }

  if (!mockActiveProject) throw new Error('No active project');
  let foundSeg = null;
  for (const t of mockActiveProject.tracks) {
    const s = t.segments.find((seg) => seg.id === segmentId);
    if (s) {
      foundSeg = s;
      break;
    }
  }
  if (!foundSeg) throw new Error(`Segment ${segmentId} not found`);

  const trimmedUs = foundSeg.sourceOutUs - foundSeg.sourceInUs;
  const targetStart = foundSeg.originalCueStartUs ?? foundSeg.timelineStartUs;
  const targetEnd = foundSeg.originalCueEndUs ?? (targetStart + trimmedUs);
  const targetDurationUs = targetEnd - targetStart;
  const requestedStartOffsetUs = manualOffsetUs ?? (foundSeg.startOffsetUs ?? 0);
  const effectiveTargetDurationUs = requestedStartOffsetUs >= 0
    ? Math.max(0, targetDurationUs - requestedStartOffsetUs)
    : targetDurationUs + Math.abs(requestedStartOffsetUs);

  let proposedTempo = 1.0;
  let paddingMode: SilencePaddingMode = manualPadding ?? config?.defaultPaddingMode ?? 'padEnd';
  let paddingBeforeUs = 0;
  let paddingAfterUs = 0;
  let requiresManualReview = false;
  let selectedStrategy = 'Original tempo preserved';

  if (manualTempo) {
    proposedTempo = manualTempo;
    selectedStrategy = `Manual tempo override (${manualTempo}x)`;
  } else if (trimmedUs > effectiveTargetDurationUs) {
    const ideal = trimmedUs / effectiveTargetDurationUs;
    proposedTempo = Math.round(ideal * 100) / 100;
    if (ideal > 1.25) {
      requiresManualReview = true;
      selectedStrategy = `Tempo (${proposedTempo}x) exceeds auto limit; manual review recommended`;
    } else {
      selectedStrategy = `Pitch-preserving speedup (${proposedTempo}x)`;
    }
  } else if (trimmedUs < effectiveTargetDurationUs) {
    const shortfall = effectiveTargetDurationUs - trimmedUs;
    if (paddingMode === 'padEnd') paddingAfterUs = shortfall;
    else if (paddingMode === 'padStart') paddingBeforeUs = shortfall;
    else if (paddingMode === 'centered') {
      paddingBeforeUs = Math.floor(shortfall / 2);
      paddingAfterUs = shortfall - paddingBeforeUs;
    }
    selectedStrategy = `Speech shorter by ${(shortfall / 1000000).toFixed(2)}s; padded with silence`;
  }

  return {
    segmentId,
    originalAssetId: foundSeg.audioAssetId,
    originalAssetHash: 'mock-orig-hash',
    sourceTrimmedDurationUs: trimmedUs,
    targetStartUs: targetStart,
    targetEndUs: targetEnd,
    targetDurationUs,
    requestedStartOffsetUs,
    effectiveTargetDurationUs,
    proposedTempo,
    paddingMode,
    paddingBeforeUs,
    paddingAfterUs,
    selectedStrategy,
    requiresManualReview,
    warnings: [],
    algorithmVersion: 1,
  };
}

/**
 * Processes audio alignment via FFmpeg atempo and silence padding.
 */
export async function processDubbingAlignment(
  plan: DubbingAlignmentPlan
): Promise<[DubbingAlignmentResult, DubbingAudioAsset]> {
  if (isTauriEnvironment()) {
    return await invoke<[DubbingAlignmentResult, DubbingAudioAsset]>('process_dubbing_alignment', { plan });
  }

  const alignedAssetId = `aligned-mock-${Date.now()}`;
  const tempoDur = Math.round(plan.sourceTrimmedDurationUs / plan.proposedTempo);
  const actualDurationUs = tempoDur + plan.paddingBeforeUs + plan.paddingAfterUs;

  const alignedAsset: DubbingAudioAsset = {
    assetId: alignedAssetId,
    filePath: `/mock/path/${alignedAssetId}.wav`,
    fileName: `${alignedAssetId}.wav`,
    sha256: 'mock-aligned-sha256',
    sampleRate: 22050,
    channelCount: 1,
    durationUs: actualDurationUs,
    sizeBytes: Math.round((actualDurationUs / 1000000) * 44100),
    voiceId: 'mock-voice',
    createdAtEpochMs: Date.now(),
  };

  const result: DubbingAlignmentResult = {
    alignmentId: `align-res-${Date.now()}`,
    segmentId: plan.segmentId,
    originalAssetId: plan.originalAssetId,
    alignedAssetId,
    actualOutputDurationUs: actualDurationUs,
    effectivePlacementStartUs: plan.targetStartUs + Math.max(0, plan.requestedStartOffsetUs),
    effectivePlacementEndUs: plan.targetStartUs + Math.max(0, plan.requestedStartOffsetUs) + actualDurationUs,
    processingMethod: `atempo=${plan.proposedTempo.toFixed(2)}, pad=${plan.paddingBeforeUs + plan.paddingAfterUs}us`,
    measuredDurationErrorUs: actualDurationUs - plan.effectiveTargetDurationUs,
    tempoFactorApplied: plan.proposedTempo,
    paddingBeforeAppliedUs: plan.paddingBeforeUs,
    paddingAfterAppliedUs: plan.paddingAfterUs,
    diagnostics: [],
    status: 'aligned',
    createdAtEpochMs: Date.now(),
  };

  return [result, alignedAsset];
}

/**
 * Applies alignment plan and result to timeline.
 */
export async function applyDubbingAlignment(
  segmentId: string,
  plan: DubbingAlignmentPlan,
  result: DubbingAlignmentResult,
  alignedAsset: DubbingAudioAsset
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('apply_dubbing_alignment', {
      segmentId,
      plan,
      result,
      alignedAsset,
    });
  }

  return await applyDubbingOperation({
    type: 'applyAlignment',
    segmentId,
    plan,
    result,
    alignedAsset,
  });
}

/**
 * Restores original unaligned TTS speech audio for a segment.
 */
export async function restoreOriginalDubbingAudio(segmentId: string): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('restore_original_dubbing_audio', { segmentId });
  }

  return await applyDubbingOperation({
    type: 'restoreOriginalAudio',
    segmentId,
  });
}

/**
 * Switches between original and aligned audio playback mode.
 */
export async function setDubbingAlignmentMode(
  segmentId: string,
  mode: DubbingAlignmentMode
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('set_dubbing_alignment_mode', { segmentId, mode });
  }

  return await applyDubbingOperation({
    type: 'setAlignmentMode',
    segmentId,
    mode,
  });
}

/**
 * Sets manual start offset in microseconds.
 */
export async function setDubbingStartOffset(
  segmentId: string,
  offsetUs: number
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('set_dubbing_start_offset', { segmentId, offsetUs });
  }

  return await applyDubbingOperation({
    type: 'setStartOffset',
    segmentId,
    offsetUs,
  });
}

/**
 * Evaluates duration alignment diagnostics for the active project.
 */
export async function getDubbingAlignmentDiagnostics(
  config?: AlignmentConfig
): Promise<DubbingAlignmentFinding[]> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingAlignmentFinding[]>('get_dubbing_alignment_diagnostics', { config });
  }

  if (!mockActiveProject) return [];
  const findings: DubbingAlignmentFinding[] = [];
  const tolerance = config?.toleranceUs ?? 25000;

  for (const track of mockActiveProject.tracks) {
    for (const seg of track.segments) {
      const dur = seg.sourceOutUs - seg.sourceInUs;
      const targetStart = seg.originalCueStartUs ?? seg.timelineStartUs;
      const targetEnd = seg.originalCueEndUs ?? (targetStart + dur);
      const targetDur = targetEnd - targetStart;
      const diff = dur - targetDur;

      if (diff > tolerance) {
        findings.push({
          code: 'speechLongerThanCue',
          severity: 'warning',
          segmentId: seg.id,
          message: `Speech duration overruns subtitle window by +${Math.round(diff / 1000)}ms`,
          durationDiffUs: diff,
        });
      }
    }
  }

  return findings;
}

/**
 * Batch aligns multiple segments.
 */
export async function batchAlignDubbingSegments(
  segmentIds?: string[],
  config?: AlignmentConfig
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('batch_align_dubbing_segments', { segmentIds, config });
  }

  if (!mockActiveProject) throw new Error('No active project');
  const candidateIds = segmentIds || mockActiveProject.tracks.flatMap((t) => t.segments.map((s) => s.id));

  for (const id of candidateIds) {
    try {
      const plan = await planDubbingAlignment(id, config);
      if (!plan.requiresManualReview) {
        const [result, alignedAsset] = await processDubbingAlignment(plan);
        await applyDubbingAlignment(id, plan, result, alignedAsset);
      }
    } catch {
      // Ignore individual failures in mock batch
    }
  }

  return { ...mockActiveProject };
}

/**
 * Default initial mix configuration.
 */
export function createDefaultMixConfiguration(projectId: string): DubbingMixConfiguration {
  return {
    schemaVersion: 1,
    projectId,
    originalAudioEnabled: true,
    originalAudioGainDb: -6.0,
    dubbingMasterGainDb: 0.0,
    trackSettings: {},
    duckingEnabled: true,
    duckingAttenuationDb: -12.0,
    duckingAttackMs: 50,
    duckingReleaseMs: 250,
    duckingHoldMs: 100,
    fadeInMs: 0,
    fadeOutMs: 0,
    outputSampleRate: 48000,
    outputChannels: 2,
    preset: 'balanced',
    limiterEnabled: true,
    peakCeilingDb: -1.0,
    updatedAtEpochMs: Date.now(),
  };
}

/**
 * Retrieves the mixing configuration for the active dubbing project.
 */
export async function getDubbingMixConfiguration(): Promise<DubbingMixConfiguration> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixConfiguration>('get_dubbing_mix_configuration');
  }

  if (!mockActiveProject) throw new Error('No active project');
  if (!mockActiveProject.mixConfiguration) {
    mockActiveProject.mixConfiguration = createDefaultMixConfiguration(mockActiveProject.id);
  }
  return { ...mockActiveProject.mixConfiguration };
}

/**
 * Updates the mixing configuration on the active dubbing project.
 */
export async function updateDubbingMixConfiguration(
  configuration: DubbingMixConfiguration
): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('update_dubbing_mix_configuration', { configuration });
  }

  if (!mockActiveProject) throw new Error('No active project');
  mockActiveProject.mixConfiguration = { ...configuration, updatedAtEpochMs: Date.now() };
  mockActiveProject.mixResult = undefined; // Invalidate stale result
  return { ...mockActiveProject };
}

/**
 * Inspects readiness to execute an audio mixdown.
 */
export async function getDubbingMixReadiness(
  mediaPathOverride?: string
): Promise<DubbingMixReadiness> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixReadiness>('get_dubbing_mix_readiness', { mediaPathOverride });
  }

  if (!mockActiveProject) {
    return {
      isReady: false,
      hasOriginalAudio: false,
      totalSpeechSegments: 0,
      readySpeechSegments: 0,
      missingSpeechSegments: 0,
      hasExistingMix: false,
      isMixStale: false,
      warnings: ['No active dubbing project'],
    };
  }

  const total = mockActiveProject.tracks.reduce((acc, t) => acc + t.segments.length, 0);
  const ready = total;
  const hasExisting = !!mockActiveProject.mixResult;

  return {
    isReady: total > 0,
    hasOriginalAudio: !!mockActiveProject.mediaReference,
    originalAudioPath: mockActiveProject.mediaReference,
    totalSpeechSegments: total,
    readySpeechSegments: ready,
    missingSpeechSegments: 0,
    hasExistingMix: hasExisting,
    isMixStale: false,
    warnings: [],
  };
}

/**
 * Generates the deterministic mixdown plan.
 */
export async function planDubbingMix(
  mediaPathOverride?: string
): Promise<DubbingMixPlan> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixPlan>('plan_dubbing_mix', { mediaPathOverride });
  }

  if (!mockActiveProject) throw new Error('No active project');
  const config = mockActiveProject.mixConfiguration || createDefaultMixConfiguration(mockActiveProject.id);

  return {
    planId: `plan-${Date.now()}`,
    projectId: mockActiveProject.id,
    timelineDurationUs: mockActiveProject.timelineDurationUs,
    hasOriginalAudio: !!mockActiveProject.mediaReference,
    originalAudioPath: mockActiveProject.mediaReference,
    originalAudioSampleRate: 48000,
    originalAudioChannels: 2,
    originalGainDb: config.originalAudioGainDb,
    dubbingMasterGainDb: config.dubbingMasterGainDb,
    segments: [],
    duckingIntervals: [],
    outputSampleRate: 48000,
    outputChannels: 2,
    filterGraphDesc: 'mock_filter_graph',
    warnings: [],
  };
}

/**
 * Initiates an offline multitrack mixdown render.
 */
export async function startDubbingMixdown(
  mediaPathOverride?: string
): Promise<DubbingMixResult> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixResult>('start_dubbing_mixdown', { mediaPathOverride });
  }

  if (!mockActiveProject) throw new Error('No active project');

  const result: DubbingMixResult = {
    resultId: `mix-${Date.now()}`,
    projectId: mockActiveProject.id,
    mixAssetId: `mix_asset_${Date.now()}`,
    filePath: `/tmp/mock_mixdown_${mockActiveProject.id}.wav`,
    sha256: 'mock_sha256_hash_1234567890abcdef',
    actualDurationUs: mockActiveProject.timelineDurationUs,
    sampleRate: 48000,
    channels: 2,
    bitDepth: 16,
    fileSizeBytes: 384000,
    peakDbfs: -1.0,
    rmsDbfs: -18.2,
    integratedLufs: -19.5,
    clippingSamples: 0,
    warnings: [],
    createdAtEpochMs: Date.now(),
  };

  mockActiveProject.mixResult = result;
  return result;
}

/**
 * Queries the status of an ongoing mixdown operation.
 */
export async function getDubbingMixdownStatus(): Promise<DubbingMixdownStatus> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixdownStatus>('get_dubbing_mixdown_status');
  }

  return {
    isRendering: false,
    progress: 1.0,
    message: 'Mixdown complete',
  };
}

/**
 * Cancels an active mixdown process.
 */
export async function cancelDubbingMixdown(): Promise<void> {
  if (isTauriEnvironment()) {
    await invoke<void>('cancel_dubbing_mixdown');
  }
}

/**
 * Retrieves the currently active mixdown result.
 */
export async function getDubbingMixdownResult(): Promise<DubbingMixResult | null> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingMixResult | null>('get_dubbing_mixdown_result');
  }

  return mockActiveProject?.mixResult || null;
}

/**
 * Measures acoustics, loudness, peaks, and clipping of an audio file.
 */
export async function analyzeDubbingMixAudio(filePath: string): Promise<AudioAnalysisReport> {
  if (isTauriEnvironment()) {
    return await invoke<AudioAnalysisReport>('analyze_dubbing_mix_audio', { filePath });
  }

  return {
    sampleRate: 48000,
    channels: 2,
    bitDepth: 16,
    durationUs: 5000000,
    peakDbfs: -1.0,
    rmsDbfs: -18.2,
    integratedLufs: -19.5,
    clippingSamples: 0,
  };
}

/**
 * Restores configuration to a preset.
 */
export async function restoreDubbingMixConfiguration(preset: MixPreset): Promise<DubbingProject> {
  if (isTauriEnvironment()) {
    return await invoke<DubbingProject>('restore_dubbing_mix_configuration', { preset });
  }

  if (!mockActiveProject) throw new Error('No active project');
  const [origGain, dubGain, duckAtt] = (() => {
    switch (preset) {
      case 'voiceFocus': return [-12.0, 1.5, -18.0] as const;
      case 'preserveOriginal': return [0.0, -1.0, -6.0] as const;
      case 'balanced':
      default:
        return [-6.0, 0.0, -12.0] as const;
    }
  })();

  const config = createDefaultMixConfiguration(mockActiveProject.id);
  config.preset = preset;
  config.originalAudioGainDb = origGain;
  config.dubbingMasterGainDb = dubGain;
  config.duckingAttenuationDb = duckAtt;

  mockActiveProject.mixConfiguration = config;
  mockActiveProject.mixResult = undefined;
  return { ...mockActiveProject };
}

