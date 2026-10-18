import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import type { SubtitleDocument, SubtitleTrack } from '../../types/subtitles';
import type { MediaAsset } from '../../types/media';
import type {
  DubbingDiagnosticFinding,
  DubbingProject,
  DubbingSegment,
} from '../../types/dubbing';
import {
  createDubbingProject,
  generateDubbingSegments,
  regenerateDubbingSegment,
  applyDubbingOperation,
  undoDubbingOperation,
  redoDubbingOperation,
  getDubbingDiagnostics,
  loadDubbingFromDocument,
  saveDubbingToDocument,
  getDubbingAssetUrl,
  batchAlignDubbingSegments,
} from '../../lib/dubbing';
import { DubbingSegmentClip } from './DubbingSegmentClip';
import { DubbingAlignmentInspector } from './DubbingAlignmentInspector';
import { DubbingMixerPanel } from './DubbingMixerPanel';
import { Button } from '../../components/ui/Button';
import {
  SparklesIcon,
  UndoIcon,
  RedoIcon,
  CheckIcon,
  AlertTriangleIcon,
  VolumeIcon,
} from '../../components/ui/Icons';

export interface DubbingTimelinePanelProps {
  document: SubtitleDocument | null;
  activeTrack: SubtitleTrack | null;
  asset: MediaAsset | null;
  currentTime: number;
  onSeek: (seconds: number) => void;
  onDocumentChange?: (doc: SubtitleDocument | null) => void;
  onOpenVoiceLibrary?: () => void;
  onOpenModelManager?: () => void;
}

export function DubbingTimelinePanel({
  document,
  activeTrack,
  asset,
  currentTime,
  onSeek,
  onDocumentChange,
}: DubbingTimelinePanelProps): React.ReactElement {
  const [project, setProject] = useState<DubbingProject | null>(null);
  const [selectedSegmentId, setSelectedSegmentId] = useState<string | null>(null);
  const [selectedTrackId, setSelectedTrackId] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<DubbingDiagnosticFinding[]>([]);
  const [showDiagnosticsDrawer, setShowDiagnosticsDrawer] = useState<boolean>(false);
  const [zoomLevel, setZoomLevel] = useState<number>(1.0); // 0.25x to 4x
  const [isGenerating, setIsGenerating] = useState<boolean>(false);
  const [isBatchAligning, setIsBatchAligning] = useState<boolean>(false);
  const [generationFeedback, setGenerationFeedback] = useState<string | null>(null);
  const [canUndo, setCanUndo] = useState<boolean>(false);
  const [canRedo, setCanRedo] = useState<boolean>(false);
  const [showMixerPanel, setShowMixerPanel] = useState<boolean>(false);

  // Selected segment helper
  const selectedSegment = useMemo(() => {
    if (!project || !selectedSegmentId) return null;
    return (
      project.tracks.flatMap((t) => t.segments).find((s) => s.id === selectedSegmentId) || null
    );
  }, [project, selectedSegmentId]);

  // Audition playback state
  const audioAuditionRef = useRef<HTMLAudioElement | null>(null);
  const [isPlayingAudition, setIsPlayingAudition] = useState<boolean>(false);

  // Timeline viewport reference
  const timelineViewportRef = useRef<HTMLDivElement>(null);

  // Base scaling: 50,000 microseconds per pixel at 1.0x zoom (20px per second)
  const microsPerPixel = useMemo(() => {
    return Math.max(5_000, Math.round(50_000 / zoomLevel));
  }, [zoomLevel]);

  // Total timeline duration
  const totalDurationUs = useMemo(() => {
    const assetDurUs = asset?.metadata?.container?.durationMicros || 60_000_000;
    const projectDurUs = project ? project.timelineDurationUs : 0;
    return Math.max(assetDurUs, projectDurUs, 30_000_000);
  }, [asset, project]);

  const timelineWidthPx = Math.ceil(totalDurationUs / microsPerPixel);

  // 1. Initialize or load project
  const loadProject = useCallback(async () => {
    if (!document) {
      setProject(null);
      return;
    }

    try {
      // Check if project is embedded in document metadata
      const embedded = await loadDubbingFromDocument(document);
      if (embedded) {
        setProject(embedded);
        if (embedded.tracks.length > 0) {
          setSelectedTrackId(embedded.tracks[0].id);
        }
      } else if (activeTrack) {
        // Automatically scaffold project from active subtitle track
        const newProj = await createDubbingProject(document, activeTrack.id, activeTrack.language);
        setProject(newProj);
        if (newProj.tracks.length > 0) {
          setSelectedTrackId(newProj.tracks[0].id);
        }
      }
    } catch (err) {
      setGenerationFeedback(`Error loading dubbing project: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [document, activeTrack]);

  useEffect(() => {
    loadProject();
  }, [loadProject]);

  // Refresh diagnostics when project changes
  useEffect(() => {
    if (project) {
      getDubbingDiagnostics(document || undefined).then((findings) => {
        setDiagnostics(findings);
      });
    } else {
      setDiagnostics([]);
    }
  }, [project, document]);

  // 2. Synthesize cues
  const handleGenerateSpeech = async () => {
    if (!project || !document || !activeTrack) return;
    const targetDubTrack = selectedTrackId
      ? project.tracks.find((t) => t.id === selectedTrackId) || project.tracks[0]
      : project.tracks[0];

    if (!targetDubTrack) return;

    setIsGenerating(true);
    setGenerationFeedback('Synthesizing speech cues via Piper offline engine...');

    try {
      const updated = await generateDubbingSegments(targetDubTrack.id, document);
      setProject(updated);
      setCanUndo(true);
      setGenerationFeedback(
        `Successfully generated speech segments for ${targetDubTrack.displayName}!`
      );
      setTimeout(() => setGenerationFeedback(null), 4000);
    } catch (err) {
      setGenerationFeedback(`Synthesis failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsGenerating(false);
    }
  };

  // 3. Regenerate selected segment
  const handleRegenerateSegment = async () => {
    if (!project || !selectedSegmentId) return;
    setIsGenerating(true);
    setGenerationFeedback('Regenerating speech segment...');

    try {
      const updated = await regenerateDubbingSegment(selectedSegmentId);
      setProject(updated);
      setCanUndo(true);
      setGenerationFeedback('Segment regenerated successfully!');
      setTimeout(() => setGenerationFeedback(null), 3000);
    } catch (err) {
      setGenerationFeedback(`Regeneration failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsGenerating(false);
    }
  };

  // 4. Operations (Move, Trim, Split, Delete, Duplicate)
  const handleMoveSegment = async (segmentId: string, newTimelineStartUs: number) => {
    if (!project) return;
    try {
      const updated = await applyDubbingOperation({
        type: 'moveSegment',
        segmentId,
        newTimelineStartUs,
      });
      setProject(updated);
      setCanUndo(true);
    } catch (err) {
      console.error('Failed to move segment:', err);
    }
  };

  const handleTrimStart = async (segmentId: string, newSourceInUs: number) => {
    if (!project) return;
    try {
      const updated = await applyDubbingOperation({
        type: 'trimSegmentStart',
        segmentId,
        newSourceInUs,
      });
      setProject(updated);
      setCanUndo(true);
    } catch (err) {
      console.error('Failed to trim segment start:', err);
    }
  };

  const handleTrimEnd = async (segmentId: string, newSourceOutUs: number) => {
    if (!project) return;
    try {
      const updated = await applyDubbingOperation({
        type: 'trimSegmentEnd',
        segmentId,
        newSourceOutUs,
      });
      setProject(updated);
      setCanUndo(true);
    } catch (err) {
      console.error('Failed to trim segment end:', err);
    }
  };

  const handleSplitSegment = async (segmentId: string) => {
    if (!project) return;
    const playheadUs = Math.round(currentTime * 1_000_000);
    try {
      const updated = await applyDubbingOperation({
        type: 'splitSegment',
        segmentId,
        splitTimelineUs: playheadUs,
      });
      setProject(updated);
      setCanUndo(true);
    } catch (err) {
      console.error('Split failed:', err);
    }
  };

  const handleDeleteSegment = async (segmentId: string) => {
    if (!project) return;
    try {
      const updated = await applyDubbingOperation({
        type: 'deleteSegment',
        segmentId,
      });
      setProject(updated);
      setSelectedSegmentId(null);
      setCanUndo(true);
    } catch (err) {
      console.error('Delete failed:', err);
    }
  };

  const handleDuplicateSegment = async (segmentId: string) => {
    if (!project) return;
    try {
      const updated = await applyDubbingOperation({
        type: 'duplicateSegment',
        segmentId,
        offsetUs: 1_000_000,
      });
      setProject(updated);
      setCanUndo(true);
    } catch (err) {
      console.error('Duplicate failed:', err);
    }
  };

  // 5. Track Controls
  const handleToggleMute = async (trackId: string, currentMute: boolean) => {
    if (!project) return;
    const updated = await applyDubbingOperation({
      type: 'setTrackMute',
      trackId,
      isMuted: !currentMute,
    });
    setProject(updated);
    setCanUndo(true);
  };

  const handleToggleSolo = async (trackId: string, currentSolo: boolean) => {
    if (!project) return;
    const updated = await applyDubbingOperation({
      type: 'setTrackSolo',
      trackId,
      isSolo: !currentSolo,
    });
    setProject(updated);
    setCanUndo(true);
  };

  // 6. Undo / Redo
  const handleUndo = async () => {
    const undone = await undoDubbingOperation();
    if (undone) {
      setProject(undone);
      setCanRedo(true);
    } else {
      setCanUndo(false);
    }
  };

  const handleRedo = async () => {
    const redone = await redoDubbingOperation();
    if (redone) {
      setProject(redone);
      setCanUndo(true);
    } else {
      setCanRedo(false);
    }
  };

  // 7. Save to Document
  const handleSaveToDocument = async () => {
    if (!document || !project) return;
    try {
      const updatedDoc = await saveDubbingToDocument(document);
      if (onDocumentChange) {
        onDocumentChange(updatedDoc);
      }
      setGenerationFeedback('Dubbing project saved to subtitle document!');
      setTimeout(() => setGenerationFeedback(null), 3000);
    } catch (err) {
      setGenerationFeedback(`Save failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  // 8. Audition segment
  const handleAudition = async (segment: DubbingSegment) => {
    try {
      const url = await getDubbingAssetUrl(segment.audioAssetId);
      if (audioAuditionRef.current) {
        audioAuditionRef.current.pause();
        audioAuditionRef.current.src = url;
        audioAuditionRef.current.currentTime = segment.sourceInUs / 1_000_000;
        audioAuditionRef.current.play();
        setIsPlayingAudition(true);
      }
    } catch (err) {
      console.error('Audition failed:', err);
    }
  };

  // 9. Dubbing Alignment Operations
  const handleBatchAutoFit = async () => {
    if (!project) return;
    const allSegments = project.tracks.flatMap((t) => t.segments);
    const eligibleIds = allSegments.filter((s) => s.audioAssetId).map((s) => s.id);
    if (eligibleIds.length === 0) {
      setGenerationFeedback('No synthesized speech segments found to align.');
      return;
    }
    setIsBatchAligning(true);
    setGenerationFeedback(`Running AutoFit alignment on ${eligibleIds.length} segments...`);
    try {
      const updated = await batchAlignDubbingSegments(eligibleIds);
      setProject(updated);
      setCanUndo(true);
      setGenerationFeedback('Batch alignment completed successfully!');
      setTimeout(() => setGenerationFeedback(null), 4000);
    } catch (err) {
      setGenerationFeedback(`Batch alignment failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsBatchAligning(false);
    }
  };

  // Playhead position in pixels
  const playheadPx = Math.round((currentTime * 1_000_000) / microsPerPixel);

  // Time ruler ticks
  const rulerTicks = useMemo(() => {
    const totalSecs = Math.ceil(totalDurationUs / 1_000_000);
    const stepSecs = zoomLevel < 0.5 ? 5 : zoomLevel < 1.5 ? 2 : 1;
    const ticks: { sec: number; x: number; label: string }[] = [];

    for (let s = 0; s <= totalSecs; s += stepSecs) {
      const x = (s * 1_000_000) / microsPerPixel;
      const m = Math.floor(s / 60);
      const remS = s % 60;
      const label = `${m}:${remS.toString().padStart(2, '0')}`;
      ticks.push({ sec: s, x, label });
    }
    return ticks;
  }, [totalDurationUs, microsPerPixel, zoomLevel]);

  // Click on ruler to seek
  const handleRulerClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickUs = clickX * microsPerPixel;
    onSeek(clickUs / 1_000_000);
  };

  return (
    <div className="flex flex-col h-full bg-zinc-950 text-zinc-100 select-none overflow-hidden">
      {/* Hidden Audio element for audition */}
      <audio
        ref={audioAuditionRef}
        onEnded={() => setIsPlayingAudition(false)}
        onError={() => setIsPlayingAudition(false)}
        className="hidden"
      />

      {/* Top Controls Toolbar */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800 bg-zinc-900/60 backdrop-blur-xs shrink-0">
        <div className="flex items-center space-x-2">
          <Button
            size="xs"
            variant="primary"
            onClick={handleGenerateSpeech}
            disabled={!project || isGenerating || !activeTrack}
            className="bg-indigo-600 hover:bg-indigo-500 font-medium"
          >
            <SparklesIcon size={12} className="mr-1.5" />
            {isGenerating ? 'Synthesizing...' : 'Generate AI Speech'}
          </Button>

          <Button
            size="xs"
            variant="secondary"
            onClick={handleBatchAutoFit}
            disabled={!project || isBatchAligning || isGenerating}
            title="Automatically analyze and time-stretch/pad speech segments to fit subtitle cues"
            className="text-xs"
          >
            <SparklesIcon size={12} className="mr-1.5 text-amber-400" />
            {isBatchAligning ? 'Aligning...' : 'Batch AutoFit'}
          </Button>

          {selectedSegmentId && (
            <Button
              size="xs"
              variant="secondary"
              onClick={handleRegenerateSegment}
              disabled={isGenerating}
              className="text-xs"
            >
              Regenerate Clip
            </Button>
          )}

          <div className="h-4 w-px bg-zinc-800 mx-1" />

          {/* Undo / Redo */}
          <Button
            size="xs"
            variant="subtle"
            onClick={handleUndo}
            disabled={!canUndo}
            title="Undo edit (Ctrl+Z)"
          >
            <UndoIcon size={13} />
          </Button>
          <Button
            size="xs"
            variant="subtle"
            onClick={handleRedo}
            disabled={!canRedo}
            title="Redo edit (Ctrl+Shift+Z)"
          >
            <RedoIcon size={13} />
          </Button>

          <Button
            size="xs"
            variant="subtle"
            onClick={handleSaveToDocument}
            title="Save timeline into project"
          >
            <CheckIcon size={13} className="mr-1 text-emerald-400" />
            <span className="text-[11px]">Save Dubbing</span>
          </Button>

          <Button
            size="xs"
            variant={showMixerPanel ? "primary" : "secondary"}
            onClick={() => {
              setShowMixerPanel(!showMixerPanel);
              if (!showMixerPanel) setSelectedSegmentId(null);
            }}
            className={`text-xs ${showMixerPanel ? 'bg-indigo-600 font-medium text-white shadow-xs' : ''}`}
            title="Open multitrack audio mixer, ducking controls, and WAV mixdown preview"
          >
            <VolumeIcon size={12} className="mr-1 text-indigo-400" />
            <span className="text-[11px]">Audio Mixer</span>
          </Button>

          {isPlayingAudition && (
            <span className="text-[10px] text-indigo-400 font-mono animate-pulse">
              Auditioning...
            </span>
          )}
        </div>

        {/* Center / Right controls */}
        <div className="flex items-center space-x-3">
          {/* Diagnostics Alert Badge */}
          {diagnostics.length > 0 && (
            <button
              type="button"
              onClick={() => setShowDiagnosticsDrawer(!showDiagnosticsDrawer)}
              className="flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] bg-amber-500/20 text-amber-300 border border-amber-500/40 hover:bg-amber-500/30 transition-colors"
            >
              <AlertTriangleIcon size={12} />
              <span>{diagnostics.length} issues</span>
            </button>
          )}

          {/* Zoom controls */}
          <div className="flex items-center space-x-1 bg-zinc-900 border border-zinc-800 rounded px-1.5 py-0.5 text-xs">
            <span className="text-[10px] text-zinc-400 mr-1">Zoom:</span>
            <button
              type="button"
              onClick={() => setZoomLevel((z) => Math.max(0.25, z - 0.25))}
              className="px-1 hover:text-white"
            >
              -
            </button>
            <span className="font-mono text-[10px] w-8 text-center">
              {Math.round(zoomLevel * 100)}%
            </span>
            <button
              type="button"
              onClick={() => setZoomLevel((z) => Math.min(4.0, z + 0.25))}
              className="px-1 hover:text-white"
            >
              +
            </button>
          </div>
        </div>
      </div>

      {/* Generation Status Feedback */}
      {generationFeedback && (
        <div className="bg-indigo-950/70 border-b border-indigo-500/30 px-3 py-1 text-xs text-indigo-200 flex items-center justify-between">
          <span>{generationFeedback}</span>
          <button
            type="button"
            onClick={() => setGenerationFeedback(null)}
            className="text-indigo-400 hover:text-white ml-2"
          >
            ✕
          </button>
        </div>
      )}

      {/* Main Timeline Workspace */}
      <div className="flex-1 flex overflow-hidden relative">
        {/* Track Headers (Left sidebar) */}
        <div className="w-56 shrink-0 border-r border-zinc-800 bg-zinc-900/50 flex flex-col z-20">
          <div className="h-8 border-b border-zinc-800 px-3 flex items-center text-[10px] font-semibold text-zinc-400 uppercase tracking-wider">
            Tracks
          </div>

          {/* Reference Rows Headers */}
          <div className="h-10 border-b border-zinc-800/80 px-3 flex items-center justify-between text-xs text-zinc-300">
            <span className="truncate font-medium">Video Reference</span>
            <span className="text-[9px] text-zinc-500 font-mono">1080p</span>
          </div>
          <div className="h-10 border-b border-zinc-800/80 px-3 flex items-center justify-between text-xs text-zinc-300">
            <span className="truncate font-medium">Original Audio</span>
            <span className="text-[9px] text-zinc-500 font-mono">Stereo</span>
          </div>
          <div className="h-10 border-b border-zinc-800/80 px-3 flex items-center justify-between text-xs text-zinc-300">
            <span className="truncate font-medium">Subtitles Reference</span>
            <span className="text-[9px] text-indigo-400 font-mono">
              {activeTrack?.cues.length || 0} cues
            </span>
          </div>

          {/* Dubbing Audio Tracks Headers */}
          {project?.tracks.map((track) => (
            <div
              key={track.id}
              onClick={() => setSelectedTrackId(track.id)}
              className={`h-16 border-b border-zinc-800 px-3 py-1 flex flex-col justify-between transition-colors cursor-pointer ${
                selectedTrackId === track.id
                  ? 'bg-indigo-950/40 border-l-2 border-l-indigo-400'
                  : 'hover:bg-zinc-800/30'
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-zinc-100 truncate">
                  {track.displayName}
                </span>
                <span className="text-[9px] px-1 py-0.2 rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 uppercase">
                  {track.language}
                </span>
              </div>
              <div className="flex items-center justify-between text-[10px] text-zinc-400">
                <span className="truncate max-w-[100px] text-zinc-400">
                  {track.assignedVoiceId ? 'Lessac (en)' : 'No Voice'}
                </span>
                <div className="flex items-center space-x-1">
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleToggleMute(track.id, track.isMuted);
                    }}
                    className={`px-1 py-0.2 rounded text-[9px] font-bold ${
                      track.isMuted
                        ? 'bg-red-500 text-white'
                        : 'bg-zinc-800 text-zinc-400 hover:text-white'
                    }`}
                  >
                    M
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleToggleSolo(track.id, track.isSolo);
                    }}
                    className={`px-1 py-0.2 rounded text-[9px] font-bold ${
                      track.isSolo
                        ? 'bg-amber-500 text-black'
                        : 'bg-zinc-800 text-zinc-400 hover:text-white'
                    }`}
                  >
                    S
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* Scrollable Timeline Grid (Right area) */}
        <div
          ref={timelineViewportRef}
          className="flex-1 overflow-x-auto overflow-y-auto relative bg-zinc-950"
        >
          <div style={{ width: `${timelineWidthPx}px` }} className="relative min-w-full">
            {/* Horizontal Time Ruler */}
            <div
              onClick={handleRulerClick}
              className="h-8 border-b border-zinc-800 bg-zinc-900/90 sticky top-0 z-30 cursor-pointer select-none"
            >
              {rulerTicks.map((tick) => (
                <div
                  key={tick.sec}
                  style={{ left: `${tick.x}px` }}
                  className="absolute top-0 bottom-0 border-l border-zinc-700/60 pl-1 text-[9px] font-mono text-zinc-400"
                >
                  {tick.label}
                </div>
              ))}
            </div>

            {/* Playhead Vertical Line */}
            <div
              style={{ left: `${playheadPx}px` }}
              className="absolute top-0 bottom-0 w-0.5 bg-rose-500 z-40 pointer-events-none shadow-sm shadow-rose-500"
            >
              <div className="w-2.5 h-2.5 bg-rose-500 rounded-full -ml-1 -mt-1 shadow-md shadow-rose-600/60" />
            </div>

            {/* Video Reference Track Lane */}
            <div className="h-10 border-b border-zinc-800/80 relative bg-zinc-900/10">
              <div
                style={{ width: `${timelineWidthPx}px` }}
                className="h-full bg-blue-950/20 border-y border-blue-500/20 px-2 flex items-center text-[10px] text-blue-300/80 font-mono"
              >
                Video Media Feed
              </div>
            </div>

            {/* Original Audio Reference Track Lane */}
            <div className="h-10 border-b border-zinc-800/80 relative bg-zinc-900/10">
              <div
                style={{ width: `${timelineWidthPx}px` }}
                className="h-full bg-emerald-950/20 border-y border-emerald-500/20 px-2 flex items-center text-[10px] text-emerald-300/80 font-mono"
              >
                Original Audio Soundtrack
              </div>
            </div>

            {/* Subtitles Reference Track Lane */}
            <div className="h-10 border-b border-zinc-800/80 relative bg-zinc-900/10">
              {activeTrack?.cues.map((cue) => {
                const cLeft = cue.startUs / microsPerPixel;
                const cWidth = Math.max(30, (cue.endUs - cue.startUs) / microsPerPixel);
                return (
                  <div
                    key={cue.id}
                    style={{ left: `${cLeft}px`, width: `${cWidth}px` }}
                    className="absolute top-1 bottom-1 rounded bg-indigo-950/40 border border-indigo-500/30 px-1.5 flex items-center text-[10px] text-indigo-300 truncate"
                  >
                    <span className="truncate">{cue.text}</span>
                  </div>
                );
              })}
            </div>

            {/* Dubbing Tracks Lanes */}
            {project?.tracks.map((track) => (
              <div
                key={track.id}
                className="h-16 border-b border-zinc-800 relative bg-zinc-950/40"
              >
                {track.segments.map((seg) => (
                  <DubbingSegmentClip
                    key={seg.id}
                    segment={seg}
                    microsPerPixel={microsPerPixel}
                    isSelected={selectedSegmentId === seg.id}
                    onSelect={setSelectedSegmentId}
                    onMove={handleMoveSegment}
                    onTrimStart={handleTrimStart}
                    onTrimEnd={handleTrimEnd}
                    onSplit={handleSplitSegment}
                    onDelete={handleDeleteSegment}
                    onDuplicate={handleDuplicateSegment}
                    onAudition={handleAudition}
                  />
                ))}
              </div>
            ))}
          </div>
        </div>

        {/* Alignment Inspector Drawer (Right sidebar) */}
        {selectedSegment && !showMixerPanel && (
          <div className="w-84 shrink-0 border-l border-zinc-800 bg-zinc-900/95 overflow-y-auto z-20">
            <DubbingAlignmentInspector
              segment={selectedSegment}
              onProjectUpdated={(updated) => {
                setProject(updated);
                setCanUndo(true);
              }}
              onClose={() => setSelectedSegmentId(null)}
            />
          </div>
        )}

        {/* Audio Mixer Panel (Right sidebar) */}
        {showMixerPanel && (
          <div className="w-88 shrink-0 border-l border-zinc-800 bg-zinc-950 overflow-y-auto z-20">
            <DubbingMixerPanel
              project={project}
              asset={asset}
              onProjectUpdated={(updated) => {
                setProject(updated);
                setCanUndo(true);
              }}
              onClose={() => setShowMixerPanel(false)}
            />
          </div>
        )}
      </div>

      {/* Diagnostics Drawer (Collapsible) */}
      {showDiagnosticsDrawer && (
        <div className="border-t border-zinc-800 bg-zinc-900/95 max-h-48 overflow-y-auto p-3 z-30 shadow-2xl">
          <div className="flex items-center justify-between pb-2 border-b border-zinc-800 text-xs font-semibold text-zinc-200">
            <span className="flex items-center space-x-1.5">
              <AlertTriangleIcon size={14} className="text-amber-400" />
              <span>Timeline Diagnostics ({diagnostics.length})</span>
            </span>
            <button
              type="button"
              onClick={() => setShowDiagnosticsDrawer(false)}
              className="text-zinc-400 hover:text-white"
            >
              ✕
            </button>
          </div>
          <div className="space-y-1.5 pt-2">
            {diagnostics.length === 0 ? (
              <p className="text-xs text-zinc-500">No issues detected on the timeline.</p>
            ) : (
              diagnostics.map((d, i) => (
                <div
                  key={i}
                  className={`text-xs p-2 rounded border flex items-center justify-between ${
                    d.severity === 'error'
                      ? 'bg-red-950/30 border-red-500/40 text-red-200'
                      : 'bg-amber-950/30 border-amber-500/40 text-amber-200'
                  }`}
                >
                  <span>{d.message}</span>
                  <span className="text-[10px] font-mono text-zinc-400 uppercase">
                    {d.code}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
