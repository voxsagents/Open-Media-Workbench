import React, { useState, useRef, useEffect } from 'react';
import type { DubbingSegment, DubbingTimelineWaveform } from '../../types/dubbing';
import { DubbingWaveform } from './DubbingWaveform';
import { getDubbingWaveform } from '../../lib/dubbing';

interface DubbingSegmentClipProps {
  segment: DubbingSegment;
  microsPerPixel: number;
  isSelected: boolean;
  onSelect: (segmentId: string) => void;
  onMove: (segmentId: string, newStartUs: number) => void;
  onTrimStart: (segmentId: string, newSourceInUs: number) => void;
  onTrimEnd: (segmentId: string, newSourceOutUs: number) => void;
  onSplit: (segmentId: string) => void;
  onDelete: (segmentId: string) => void;
  onDuplicate: (segmentId: string) => void;
  onAudition: (segment: DubbingSegment) => void;
}

export function DubbingSegmentClip({
  segment,
  microsPerPixel,
  isSelected,
  onSelect,
  onMove,
  onTrimStart,
  onTrimEnd,
  onSplit,
  onDelete,
  onDuplicate,
  onAudition,
}: DubbingSegmentClipProps): React.ReactElement {
  const [waveform, setWaveform] = useState<DubbingTimelineWaveform | null>(null);
  const clipRef = useRef<HTMLDivElement>(null);

  // Drag interaction state
  const [isDragging, setIsDragging] = useState(false);
  const [dragStartX, setDragStartX] = useState(0);
  const [dragStartUs, setDragStartUs] = useState(0);

  // Trim interaction state
  const [trimmingEdge, setTrimmingEdge] = useState<'start' | 'end' | null>(null);
  const [trimStartX, setTrimStartX] = useState(0);
  const [initialSourceInUs, setInitialSourceInUs] = useState(0);
  const [initialSourceOutUs, setInitialSourceOutUs] = useState(0);

  // Fetch real waveform on mount
  useEffect(() => {
    let isMounted = true;
    getDubbingWaveform(segment.id)
      .then((wf) => {
        if (isMounted) setWaveform(wf);
      })
      .catch(() => {
        // Fallback / missing asset handled cleanly
      });
    return () => {
      isMounted = false;
    };
  }, [segment.id, segment.audioAssetId]);

  const durationUs = Math.max(100_000, segment.sourceOutUs - segment.sourceInUs);
  const leftPx = Math.max(0, segment.timelineStartUs / microsPerPixel);
  const widthPx = Math.max(48, durationUs / microsPerPixel);

  // Overrun calculation relative to original cue
  const effectiveEndUs = segment.timelineStartUs + durationUs;
  const overrunUs = segment.originalCueEndUs
    ? effectiveEndUs - segment.originalCueEndUs
    : 0;
  const isOverrunning = overrunUs > 150_000; // > 150ms overrun
  const overrunSecs = (overrunUs / 1_000_000).toFixed(2);

  // Handle Drag Move
  const handlePointerDown = (e: React.PointerEvent) => {
    if (trimmingEdge) return;
    if (e.button !== 0) return;
    e.stopPropagation();
    onSelect(segment.id);

    setIsDragging(true);
    setDragStartX(e.clientX);
    setDragStartUs(segment.timelineStartUs);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (isDragging) {
      const deltaPx = e.clientX - dragStartX;
      const deltaUs = Math.round(deltaPx * microsPerPixel);
      const newStart = Math.max(0, dragStartUs + deltaUs);
      onMove(segment.id, newStart);
    } else if (trimmingEdge === 'start') {
      const deltaPx = e.clientX - trimStartX;
      const deltaUs = Math.round(deltaPx * microsPerPixel);
      const newIn = Math.max(0, Math.min(initialSourceInUs + deltaUs, segment.sourceOutUs - 50_000));
      onTrimStart(segment.id, newIn);
    } else if (trimmingEdge === 'end') {
      const deltaPx = e.clientX - trimStartX;
      const deltaUs = Math.round(deltaPx * microsPerPixel);
      const maxSource = waveform ? waveform.durationUs : segment.sourceOutUs + 5_000_000;
      const newOut = Math.min(
        maxSource,
        Math.max(segment.sourceInUs + 50_000, initialSourceOutUs + deltaUs)
      );
      onTrimEnd(segment.id, newOut);
    }
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (isDragging) {
      setIsDragging(false);
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
    if (trimmingEdge) {
      setTrimmingEdge(null);
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  };

  // Start Trim Edge
  const startTrim = (edge: 'start' | 'end', e: React.PointerEvent) => {
    e.stopPropagation();
    setTrimmingEdge(edge);
    setTrimStartX(e.clientX);
    setInitialSourceInUs(segment.sourceInUs);
    setInitialSourceOutUs(segment.sourceOutUs);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };

  return (
    <div
      ref={clipRef}
      role="button"
      tabIndex={0}
      aria-label={`Speech segment: ${segment.sourceText}`}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onKeyDown={(e) => {
        if (e.key === 'Delete' || e.key === 'Backspace') {
          e.stopPropagation();
          onDelete(segment.id);
        } else if (e.key === 's' || e.key === 'S') {
          e.stopPropagation();
          onSplit(segment.id);
        } else if (e.key === 'd' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          onDuplicate(segment.id);
        }
      }}
      style={{
        left: `${leftPx}px`,
        width: `${widthPx}px`,
      }}
      className={`absolute top-1 bottom-1 rounded-md select-none border transition-shadow cursor-grab active:cursor-grabbing flex flex-col justify-between overflow-hidden group ${
        isSelected
          ? 'bg-indigo-950/80 border-indigo-400 ring-2 ring-indigo-500/50 shadow-lg shadow-indigo-950/50 z-20'
          : 'bg-zinc-900/90 border-zinc-700/80 hover:border-zinc-500 z-10'
      }`}
    >
      {/* Waveform Background */}
      <div className="absolute inset-0 pointer-events-none opacity-80">
        <DubbingWaveform
          waveform={waveform}
          sourceInUs={segment.sourceInUs}
          sourceOutUs={segment.sourceOutUs}
          widthPx={widthPx}
          heightPx={46}
          color={isOverrunning ? '#f59e0b' : '#818cf8'}
        />
      </div>

      {/* Clip Header */}
      <div className="relative z-10 px-2 py-0.5 flex items-center justify-between text-[10px] bg-zinc-950/60 backdrop-blur-xs border-b border-white/5">
        <div className="flex items-center space-x-1.5 truncate max-w-[70%]">
          <span className="font-semibold text-zinc-200 truncate">{segment.sourceText}</span>
        </div>
        <div className="flex items-center space-x-1 shrink-0">
          {isOverrunning && (
            <span
              title={`Speech exceeds subtitle interval by +${overrunSecs}s`}
              className="px-1 py-0.2 rounded bg-amber-500/20 text-amber-300 font-mono text-[9px] border border-amber-500/40"
            >
              +{overrunSecs}s
            </span>
          )}
          <span className="text-zinc-400 font-mono text-[9px]">
            {(durationUs / 1_000_000).toFixed(1)}s
          </span>
        </div>
      </div>

      {/* Floating Toolbar on Hover / Selection */}
      {(isSelected || isDragging) && (
        <div className="absolute bottom-1 right-2 z-20 flex items-center space-x-1 bg-zinc-900/90 border border-zinc-700 rounded px-1.5 py-0.5 shadow-md">
          <button
            type="button"
            title="Audition speech segment"
            onClick={(e) => {
              e.stopPropagation();
              onAudition(segment);
            }}
            className="text-[10px] text-zinc-300 hover:text-white px-1"
          >
            ▶
          </button>
          <button
            type="button"
            title="Split segment"
            onClick={(e) => {
              e.stopPropagation();
              onSplit(segment.id);
            }}
            className="text-[10px] text-zinc-300 hover:text-indigo-400 px-1 border-l border-zinc-700"
          >
            ✂
          </button>
          <button
            type="button"
            title="Duplicate clip"
            onClick={(e) => {
              e.stopPropagation();
              onDuplicate(segment.id);
            }}
            className="text-[10px] text-zinc-300 hover:text-emerald-400 px-1 border-l border-zinc-700"
          >
            ⧉
          </button>
          <button
            type="button"
            title="Delete segment"
            onClick={(e) => {
              e.stopPropagation();
              onDelete(segment.id);
            }}
            className="text-[10px] text-zinc-400 hover:text-red-400 px-1 border-l border-zinc-700"
          >
            ✕
          </button>
        </div>
      )}

      {/* Left Trim Handle */}
      <div
        role="separator"
        aria-label="Trim clip start"
        onPointerDown={(e) => startTrim('start', e)}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        className="absolute left-0 top-0 bottom-0 w-2 cursor-ew-resize hover:bg-indigo-500/80 active:bg-indigo-400 transition-colors z-30 flex items-center justify-center group-hover:bg-zinc-700/50"
      >
        <div className="w-0.5 h-3 bg-white/60 rounded-full" />
      </div>

      {/* Right Trim Handle */}
      <div
        role="separator"
        aria-label="Trim clip end"
        onPointerDown={(e) => startTrim('end', e)}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        className="absolute right-0 top-0 bottom-0 w-2 cursor-ew-resize hover:bg-indigo-500/80 active:bg-indigo-400 transition-colors z-30 flex items-center justify-center group-hover:bg-zinc-700/50"
      >
        <div className="w-0.5 h-3 bg-white/60 rounded-full" />
      </div>
    </div>
  );
}
