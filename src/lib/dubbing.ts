/**
 * Client IPC bindings and local state service for Professional AI Dubbing Timeline.
 */

import { invoke } from '@tauri-apps/api/core';
import type { SubtitleDocument } from '../types/subtitles';
import type { TtsVoiceSettings } from '../types/tts';
import type {
  DubbingDiagnosticFinding,
  DubbingOperation,
  DubbingProject,
  DubbingTimelineWaveform,
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
