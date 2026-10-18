import React, { useMemo } from 'react';
import type { DubbingTimelineWaveform } from '../../types/dubbing';

interface DubbingWaveformProps {
  waveform: DubbingTimelineWaveform | null;
  sourceInUs: number;
  sourceOutUs: number;
  widthPx: number;
  heightPx?: number;
  color?: string;
}

/**
 * High-performance SVG waveform visualizer rendering real PCM audio peak bins.
 */
export function DubbingWaveform({
  waveform,
  sourceInUs,
  sourceOutUs,
  widthPx,
  heightPx = 40,
  color = '#818cf8', // Indigo-400
}: DubbingWaveformProps): React.ReactElement {
  const pathData = useMemo(() => {
    if (!waveform || waveform.bins.length === 0 || widthPx <= 0) {
      return '';
    }

    const totalDurUs = waveform.durationUs || 1;
    const startRatio = Math.max(0, Math.min(1, sourceInUs / totalDurUs));
    const endRatio = Math.max(0, Math.min(1, sourceOutUs / totalDurUs));

    const totalBins = waveform.bins.length;
    const startBinIdx = Math.floor(startRatio * totalBins);
    const endBinIdx = Math.min(totalBins, Math.ceil(endRatio * totalBins));
    const visibleBins = waveform.bins.slice(startBinIdx, endBinIdx);

    if (visibleBins.length === 0) return '';

    const midY = heightPx / 2;
    const ampScale = (heightPx / 2) * 0.9;
    const stepX = widthPx / Math.max(1, visibleBins.length);

    let upperPath = `M 0 ${midY}`;
    let lowerPath = '';

    for (let i = 0; i < visibleBins.length; i++) {
      const bin = visibleBins[i];
      const x = i * stepX;
      const yMax = midY - Math.max(0, bin.max) * ampScale;
      const yMin = midY - Math.min(0, bin.min) * ampScale;

      upperPath += ` L ${x.toFixed(1)} ${yMax.toFixed(1)}`;
      lowerPath = ` L ${x.toFixed(1)} ${yMin.toFixed(1)}` + lowerPath;
    }

    return `${upperPath} L ${widthPx} ${midY} ${lowerPath} Z`;
  }, [waveform, sourceInUs, sourceOutUs, widthPx, heightPx]);

  if (!waveform || !pathData) {
    // Subtle fallback bar when loading or no audio bins
    return (
      <div
        className="w-full h-full flex items-center justify-center opacity-30"
        style={{ height: `${heightPx}px` }}
      >
        <div className="w-full h-0.5 bg-indigo-500/40 rounded-full" />
      </div>
    );
  }

  return (
    <svg
      width={widthPx}
      height={heightPx}
      className="pointer-events-none absolute inset-0 w-full h-full"
      viewBox={`0 0 ${widthPx} ${heightPx}`}
      preserveAspectRatio="none"
    >
      <path d={pathData} fill={color} fillOpacity={0.4} stroke={color} strokeWidth={0.8} />
    </svg>
  );
}
