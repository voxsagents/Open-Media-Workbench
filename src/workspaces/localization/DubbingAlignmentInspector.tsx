import React, { useState, useEffect } from 'react';
import type {
  DubbingProject,
  DubbingSegment,
  DubbingAlignmentPlan,
  SilencePaddingMode,
} from '../../types/dubbing';
import {
  planDubbingAlignment,
  processDubbingAlignment,
  applyDubbingAlignment,
  restoreOriginalDubbingAudio,
  setDubbingAlignmentMode,
  setDubbingStartOffset,
  getDubbingAssetUrl,
} from '../../lib/dubbing';

interface DubbingAlignmentInspectorProps {
  segment: DubbingSegment;
  onProjectUpdated: (project: DubbingProject) => void;
  onSeekRequested?: (micros: number) => void;
  onClose?: () => void;
}

export const DubbingAlignmentInspector: React.FC<DubbingAlignmentInspectorProps> = ({
  segment,
  onProjectUpdated,
  onClose,
}) => {
  const [isPlanning, setIsPlanning] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [plan, setPlan] = useState<DubbingAlignmentPlan | null>(segment.alignmentPlan || null);
  const [error, setError] = useState<string | null>(null);

  // Manual adjustment form state
  const [showManual, setShowManual] = useState(false);
  const [manualTempo, setManualTempo] = useState<number>(
    segment.alignmentPlan?.proposedTempo || 1.0
  );
  const [manualPaddingMode, setManualPaddingMode] = useState<SilencePaddingMode>(
    segment.alignmentPlan?.paddingMode || 'padEnd'
  );
  const [manualOffsetMs, setManualOffsetMs] = useState<number>(
    Math.round((segment.startOffsetUs || 0) / 1000)
  );

  // A/B audio preview state
  const [originalAudioUrl, setOriginalAudioUrl] = useState<string | null>(null);
  const [alignedAudioUrl, setAlignedAudioUrl] = useState<string | null>(null);

  const trimmedDurUs = segment.sourceOutUs - segment.sourceInUs;
  const targetStartUs = segment.originalCueStartUs ?? segment.timelineStartUs;
  const targetEndUs = segment.originalCueEndUs ?? (targetStartUs + trimmedDurUs);
  const targetDurUs = targetEndUs - targetStartUs;
  const diffUs = trimmedDurUs - targetDurUs;
  const diffMs = Math.round(diffUs / 1000);

  const isOverrun = diffUs > 25_000;
  const isUnderrun = diffUs < -25_000;
  const isAligned = segment.alignedAudioAssetId && segment.alignmentMode !== 'original';

  useEffect(() => {
    setPlan(segment.alignmentPlan || null);
    setManualTempo(segment.alignmentPlan?.proposedTempo || 1.0);
    setManualPaddingMode(segment.alignmentPlan?.paddingMode || 'padEnd');
    setManualOffsetMs(Math.round((segment.startOffsetUs || 0) / 1000));
    setError(null);

    // Resolve A/B audio URLs
    const origId = segment.originalAudioAssetId || segment.audioAssetId;
    getDubbingAssetUrl(origId).then(setOriginalAudioUrl).catch(() => setOriginalAudioUrl(null));

    if (segment.alignedAudioAssetId) {
      getDubbingAssetUrl(segment.alignedAudioAssetId)
        .then(setAlignedAudioUrl)
        .catch(() => setAlignedAudioUrl(null));
    } else {
      setAlignedAudioUrl(null);
    }
  }, [segment]);

  const handleComputePlan = async (tempo?: number, padding?: SilencePaddingMode, offsetMs?: number) => {
    setIsPlanning(true);
    setError(null);
    try {
      const offsetUs = offsetMs !== undefined ? offsetMs * 1000 : manualOffsetMs * 1000;
      const computed = await planDubbingAlignment(
        segment.id,
        undefined,
        tempo ?? (showManual ? manualTempo : undefined),
        padding ?? (showManual ? manualPaddingMode : undefined),
        offsetUs
      );
      setPlan(computed);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsPlanning(false);
    }
  };

  const handleExecuteAutoFit = async () => {
    setIsProcessing(true);
    setError(null);
    try {
      // 1. Plan
      const alignmentPlan = await planDubbingAlignment(segment.id);
      setPlan(alignmentPlan);

      // 2. Process
      const [result, alignedAsset] = await processDubbingAlignment(alignmentPlan);

      // 3. Apply
      const updated = await applyDubbingAlignment(segment.id, alignmentPlan, result, alignedAsset);
      onProjectUpdated(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsProcessing(false);
    }
  };

  const handleApplyManual = async () => {
    if (!plan) {
      await handleComputePlan();
    }
    setIsProcessing(true);
    setError(null);
    try {
      const activePlan = plan || (await planDubbingAlignment(
        segment.id,
        undefined,
        manualTempo,
        manualPaddingMode,
        manualOffsetMs * 1000
      ));
      const [result, alignedAsset] = await processDubbingAlignment(activePlan);
      const updated = await applyDubbingAlignment(segment.id, activePlan, result, alignedAsset);
      onProjectUpdated(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsProcessing(false);
    }
  };

  const handleRestoreOriginal = async () => {
    setIsProcessing(true);
    setError(null);
    try {
      const updated = await restoreOriginalDubbingAudio(segment.id);
      onProjectUpdated(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsProcessing(false);
    }
  };

  const handleToggleMode = async (mode: 'original' | 'autoFit') => {
    setIsProcessing(true);
    setError(null);
    try {
      const updated = await setDubbingAlignmentMode(segment.id, mode);
      onProjectUpdated(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsProcessing(false);
    }
  };

  const handleNudgeOffset = async (deltaMs: number) => {
    const nextOffsetMs = manualOffsetMs + deltaMs;
    setManualOffsetMs(nextOffsetMs);
    try {
      const updated = await setDubbingStartOffset(segment.id, nextOffsetMs * 1000);
      onProjectUpdated(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="bg-neutral-900 border border-neutral-700/80 rounded-lg p-4 space-y-4 text-xs">
      <div className="flex items-center justify-between border-b border-neutral-800 pb-2">
        <div className="flex items-center gap-2">
          <span className="font-semibold text-neutral-200">Duration Alignment</span>
          {isAligned ? (
            <span className="bg-emerald-500/20 text-emerald-300 px-2 py-0.5 rounded text-[10px] font-mono border border-emerald-500/30">
              ALIGNED ({segment.alignmentPlan?.proposedTempo.toFixed(2)}x)
            </span>
          ) : (
            <span className="bg-neutral-800 text-neutral-400 px-2 py-0.5 rounded text-[10px] font-mono">
              ORIGINAL AUDIO
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-neutral-500 font-mono text-[11px]">ID: {segment.id.slice(0, 8)}</span>
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="text-neutral-400 hover:text-white px-1 text-xs"
              title="Close Inspector"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {error && (
        <div className="bg-red-900/30 border border-red-500/40 text-red-200 px-3 py-2 rounded text-[11px]">
          {error}
        </div>
      )}

      {/* Timing Comparison Grid */}
      <div className="grid grid-cols-3 gap-2 bg-neutral-950 p-2.5 rounded border border-neutral-800/80">
        <div>
          <div className="text-neutral-500 text-[10px]">Speech Duration</div>
          <div className="font-mono text-neutral-200 font-medium">
            {(trimmedDurUs / 1_000_000).toFixed(2)}s
          </div>
        </div>
        <div>
          <div className="text-neutral-500 text-[10px]">Subtitle Window</div>
          <div className="font-mono text-neutral-200 font-medium">
            {(targetDurUs / 1_000_000).toFixed(2)}s
          </div>
        </div>
        <div>
          <div className="text-neutral-500 text-[10px]">Duration Delta</div>
          <div
            className={`font-mono font-medium ${
              isOverrun
                ? 'text-amber-400'
                : isUnderrun
                ? 'text-sky-400'
                : 'text-emerald-400'
            }`}
          >
            {diffMs > 0 ? `+${diffMs}ms` : `${diffMs}ms`}
          </div>
        </div>
      </div>

      {/* AutoFit Action */}
      <div className="space-y-2">
        <button
          onClick={handleExecuteAutoFit}
          disabled={isProcessing || isPlanning}
          className="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-medium py-1.5 px-3 rounded flex items-center justify-center gap-2 transition"
        >
          {isProcessing ? 'Processing via FFmpeg...' : '⚡ AutoFit Duration to Subtitle'}
        </button>

        {plan && (
          <div className="bg-neutral-950/70 border border-neutral-800 p-2.5 rounded space-y-1">
            <div className="text-neutral-400 font-medium flex items-center justify-between">
              <span>Strategy:</span>
              <span className="font-mono text-indigo-300">
                {plan.proposedTempo.toFixed(2)}x Speed
              </span>
            </div>
            <div className="text-neutral-300 text-[11px]">{plan.selectedStrategy}</div>
            {plan.requiresManualReview && (
              <div className="text-amber-400 font-medium text-[10px] mt-1 bg-amber-500/10 p-1.5 rounded border border-amber-500/20">
                ⚠ Exceeds conservative speedup limit; review speech naturalness before export.
              </div>
            )}
          </div>
        )}
      </div>

      {/* A/B Audio Audition Preview */}
      <div className="space-y-2 border-t border-neutral-800 pt-3">
        <div className="flex items-center justify-between">
          <span className="font-medium text-neutral-300">A/B Audition Preview</span>
          {isAligned && (
            <div className="flex gap-1">
              <button
                onClick={() => handleToggleMode('original')}
                className={`px-2 py-0.5 rounded text-[10px] ${
                  segment.alignmentMode === 'original'
                    ? 'bg-neutral-700 text-white font-semibold'
                    : 'bg-neutral-800 text-neutral-400 hover:text-white'
                }`}
              >
                Track: Original
              </button>
              <button
                onClick={() => handleToggleMode('autoFit')}
                className={`px-2 py-0.5 rounded text-[10px] ${
                  segment.alignmentMode !== 'original'
                    ? 'bg-indigo-600 text-white font-semibold'
                    : 'bg-neutral-800 text-neutral-400 hover:text-white'
                }`}
              >
                Track: Aligned
              </button>
            </div>
          )}
        </div>

        <div className="space-y-2">
          {/* Audio A: Original */}
          <div className="bg-neutral-950 p-2 rounded border border-neutral-800">
            <div className="flex justify-between items-center mb-1 text-[11px]">
              <span className="text-neutral-400">Track A (Original Speech):</span>
              <span className="font-mono text-neutral-500">
                {(trimmedDurUs / 1_000_000).toFixed(2)}s
              </span>
            </div>
            {originalAudioUrl ? (
              <audio controls src={originalAudioUrl} className="w-full h-7" />
            ) : (
              <div className="text-neutral-500 text-[10px] italic">Loading original audio...</div>
            )}
          </div>

          {/* Audio B: Aligned */}
          {alignedAudioUrl ? (
            <div className="bg-neutral-950 p-2 rounded border border-indigo-900/40">
              <div className="flex justify-between items-center mb-1 text-[11px]">
                <span className="text-indigo-400 font-medium">Track B (Aligned Speech):</span>
                <span className="font-mono text-indigo-300">
                  {((segment.alignmentResult?.actualOutputDurationUs || trimmedDurUs) / 1_000_000).toFixed(2)}s
                </span>
              </div>
              <audio controls src={alignedAudioUrl} className="w-full h-7" />
            </div>
          ) : (
            <div className="text-neutral-500 text-[10px] italic p-1 bg-neutral-950 rounded">
              Track B (Aligned Speech): Not yet processed. Click AutoFit to generate.
            </div>
          )}
        </div>
      </div>

      {/* Advanced / Manual Controls (Collapsible) */}
      <div className="border-t border-neutral-800 pt-3">
        <button
          onClick={() => setShowManual(!showManual)}
          className="text-neutral-400 hover:text-neutral-200 flex items-center justify-between w-full font-medium"
        >
          <span>Advanced Manual Controls</span>
          <span>{showManual ? '▲' : '▼'}</span>
        </button>

        {showManual && (
          <div className="mt-3 space-y-3 bg-neutral-950 p-3 rounded border border-neutral-800">
            {/* Speed / Tempo slider */}
            <div className="space-y-1">
              <div className="flex justify-between text-[11px]">
                <span className="text-neutral-400">Pitch-Preserving Tempo:</span>
                <span className="font-mono font-medium text-neutral-200">
                  {manualTempo.toFixed(2)}x
                </span>
              </div>
              <input
                type="range"
                min="0.50"
                max="2.00"
                step="0.01"
                value={manualTempo}
                onChange={(e) => {
                  const val = parseFloat(e.target.value);
                  setManualTempo(val);
                  handleComputePlan(val, manualPaddingMode, manualOffsetMs);
                }}
                className="w-full accent-indigo-500 cursor-pointer"
              />
              <div className="flex justify-between text-[9px] text-neutral-600 font-mono">
                <span>0.50x (Slow)</span>
                <span>1.00x (Original)</span>
                <span>2.00x (Fast)</span>
              </div>
            </div>

            {/* Silence padding selector */}
            <div className="space-y-1">
              <label className="text-neutral-400 text-[11px]">Silence Padding Mode:</label>
              <select
                value={manualPaddingMode}
                onChange={(e) => {
                  const mode = e.target.value as SilencePaddingMode;
                  setManualPaddingMode(mode);
                  handleComputePlan(manualTempo, mode, manualOffsetMs);
                }}
                className="w-full bg-neutral-900 border border-neutral-700 text-neutral-200 rounded px-2 py-1 text-xs"
              >
                <option value="padEnd">Pad End (Preserve Spoken Start)</option>
                <option value="padStart">Pad Start (Delay Spoken Start)</option>
                <option value="centered">Centered (Split Evenly)</option>
                <option value="noPadding">No Padding</option>
              </select>
            </div>

            {/* Start Offset Nudge */}
            <div className="space-y-1">
              <div className="flex justify-between text-[11px]">
                <span className="text-neutral-400">Start Offset (Nudge):</span>
                <span className="font-mono text-neutral-200">
                  {manualOffsetMs > 0 ? `+${manualOffsetMs}ms` : `${manualOffsetMs}ms`}
                </span>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => handleNudgeOffset(-100)}
                  className="flex-1 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 py-1 rounded font-mono"
                >
                  -100ms
                </button>
                <button
                  onClick={() => handleNudgeOffset(-25)}
                  className="flex-1 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 py-1 rounded font-mono"
                >
                  -25ms
                </button>
                <button
                  onClick={() => handleNudgeOffset(25)}
                  className="flex-1 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 py-1 rounded font-mono"
                >
                  +25ms
                </button>
                <button
                  onClick={() => handleNudgeOffset(100)}
                  className="flex-1 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 py-1 rounded font-mono"
                >
                  +100ms
                </button>
              </div>
            </div>

            <button
              onClick={handleApplyManual}
              disabled={isProcessing}
              className="w-full bg-neutral-800 hover:bg-neutral-700 text-neutral-200 py-1.5 px-3 rounded font-medium transition"
            >
              Apply Manual Alignment
            </button>
          </div>
        )}
      </div>

      {/* Restore Button */}
      {isAligned && (
        <div className="border-t border-neutral-800 pt-2">
          <button
            onClick={handleRestoreOriginal}
            disabled={isProcessing}
            className="w-full bg-neutral-800/80 hover:bg-red-950/40 hover:text-red-300 text-neutral-400 py-1 px-3 rounded transition text-[11px]"
          >
            ↺ Restore Original Unaligned Speech
          </button>
        </div>
      )}
    </div>
  );
};
