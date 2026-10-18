import React, { useState, useEffect, useRef, useCallback } from 'react';
import type { DubbingProject, DubbingMixConfiguration, DubbingMixResult, MixPreset, DubbingMixReadiness } from '../../types/dubbing';
import type { MediaAsset } from '../../types/media';
import {
  getDubbingMixConfiguration,
  updateDubbingMixConfiguration,
  getDubbingMixReadiness,
  startDubbingMixdown,
  cancelDubbingMixdown,
  getDubbingMixdownResult,
  restoreDubbingMixConfiguration,
  getDubbingAssetUrl,
} from '../../lib/dubbing';
import { Button } from '../../components/ui/Button';
import {
  VolumeIcon,
  PlayIcon,
  PauseIcon,
  UndoIcon,
  SparklesIcon,
  AlertTriangleIcon,
} from '../../components/ui/Icons';

export interface DubbingMixerPanelProps {
  project: DubbingProject | null;
  asset: MediaAsset | null;
  onProjectUpdated?: (project: DubbingProject) => void;
  onClose?: () => void;
}

export function DubbingMixerPanel({
  project,
  asset,
  onProjectUpdated,
  onClose,
}: DubbingMixerPanelProps): React.ReactElement {
  const [config, setConfig] = useState<DubbingMixConfiguration | null>(null);
  const [mixResult, setMixResult] = useState<DubbingMixResult | null>(null);
  const [readiness, setReadiness] = useState<DubbingMixReadiness | null>(null);
  const [isRendering, setIsRendering] = useState<boolean>(false);
  const [renderProgress, setRenderProgress] = useState<number>(0);
  const [renderMessage, setRenderMessage] = useState<string>('');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // A/B Preview State: 'A' = Original Audio, 'B' = Rendered Mixdown
  const [previewTrack, setPreviewTrack] = useState<'A' | 'B'>('B');
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [playbackCurrentTime, setPlaybackCurrentTime] = useState<number>(0);
  const [playbackDuration, setPlaybackDuration] = useState<number>(0);
  const [previewAudioSrc, setPreviewAudioSrc] = useState<string>('');

  const audioPreviewRef = useRef<HTMLAudioElement | null>(null);

  // Load configuration and existing mixdown result
  const refreshState = useCallback(async () => {
    if (!project) return;
    try {
      const cfg = await getDubbingMixConfiguration();
      setConfig(cfg);
      const res = await getDubbingMixdownResult();
      setMixResult(res);
      const ready = await getDubbingMixReadiness(asset?.source?.canonicalPath);
      setReadiness(ready);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  }, [project, asset]);

  useEffect(() => {
    refreshState();
  }, [refreshState]);

  // Asynchronously resolve preview audio source URL
  useEffect(() => {
    let isMounted = true;
    const loadUrl = async () => {
      if (previewTrack === 'B') {
        if (!mixResult) {
          if (isMounted) setPreviewAudioSrc('');
          return;
        }
        try {
          const url = await getDubbingAssetUrl(mixResult.filePath);
          if (isMounted) setPreviewAudioSrc(url);
        } catch {
          if (isMounted) setPreviewAudioSrc('');
        }
      } else {
        if (!asset?.source?.canonicalPath) {
          if (isMounted) setPreviewAudioSrc('');
          return;
        }
        try {
          const url = await getDubbingAssetUrl(asset.source.canonicalPath);
          if (isMounted) setPreviewAudioSrc(url);
        } catch {
          if (isMounted) setPreviewAudioSrc('');
        }
      }
    };
    loadUrl();
    return () => {
      isMounted = false;
    };
  }, [previewTrack, mixResult, asset]);

  // Handle configuration changes
  const handleUpdateConfig = async (updated: Partial<DubbingMixConfiguration>) => {
    if (!config || !project) return;
    const newConfig: DubbingMixConfiguration = {
      ...config,
      ...updated,
      preset: updated.preset || 'custom',
    };
    setConfig(newConfig);

    try {
      const updatedProject = await updateDubbingMixConfiguration(newConfig);
      setMixResult(null); // Invalidated
      onProjectUpdated?.(updatedProject);
      const ready = await getDubbingMixReadiness(asset?.source?.canonicalPath);
      setReadiness(ready);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  // Handle preset selection
  const handlePresetChange = async (preset: MixPreset) => {
    if (!project) return;
    try {
      const updatedProject = await restoreDubbingMixConfiguration(preset);
      if (updatedProject.mixConfiguration) {
        setConfig(updatedProject.mixConfiguration);
      }
      setMixResult(null);
      onProjectUpdated?.(updatedProject);
      const ready = await getDubbingMixReadiness(asset?.source?.canonicalPath);
      setReadiness(ready);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  // Track gain/mute/solo changes
  const handleTrackSettingChange = (
    trackId: string,
    field: 'gainDb' | 'isMuted' | 'isSolo' | 'isEnabled',
    val: number | boolean
  ) => {
    if (!config) return;
    const current = config.trackSettings[trackId] || {
      trackId,
      isEnabled: true,
      isMuted: false,
      isSolo: false,
      gainDb: 0.0,
      pan: 0.0,
    };

    const updated = {
      ...current,
      [field]: val,
    };

    handleUpdateConfig({
      trackSettings: {
        ...config.trackSettings,
        [trackId]: updated,
      },
    });
  };

  // Execute mixdown
  const handleStartMixdown = async () => {
    setIsRendering(true);
    setRenderProgress(0.1);
    setRenderMessage('Preparing multitrack mixing graph...');
    setErrorMessage(null);

    // Stop active preview
    if (audioPreviewRef.current) {
      audioPreviewRef.current.pause();
      setIsPlaying(false);
    }

    try {
      const res = await startDubbingMixdown(asset?.source?.canonicalPath);
      setMixResult(res);
      setRenderProgress(1.0);
      setRenderMessage('Mixdown successfully generated and verified.');
      setPreviewTrack('B');
      await refreshState();
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRendering(false);
    }
  };

  // Cancel mixdown
  const handleCancelMixdown = async () => {
    try {
      await cancelDubbingMixdown();
      setIsRendering(false);
      setRenderMessage('Mixdown cancelled.');
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  // Audio preview play/pause toggle
  const togglePlayPreview = () => {
    if (!audioPreviewRef.current || !previewAudioSrc) return;
    if (isPlaying) {
      audioPreviewRef.current.pause();
      setIsPlaying(false);
    } else {
      audioPreviewRef.current.play().catch((err) => {
        setErrorMessage(`Preview playback error: ${err.message}`);
      });
      setIsPlaying(true);
    }
  };

  const handleSeekPreview = (seconds: number) => {
    if (!audioPreviewRef.current) return;
    audioPreviewRef.current.currentTime = seconds;
    setPlaybackCurrentTime(seconds);
  };

  if (!project) {
    return (
      <div className="flex flex-col items-center justify-center p-8 text-zinc-400 text-xs">
        <VolumeIcon size={24} className="mb-2 text-zinc-600" />
        <p>No active dubbing project loaded.</p>
      </div>
    );
  }

  const isMixStale = !mixResult && (project.mixResult !== undefined);

  return (
    <div className="flex flex-col h-full bg-zinc-950 text-zinc-100 select-none overflow-y-auto border-l border-zinc-800">
      {/* Hidden Audio Player for A/B Preview */}
      <audio
        ref={audioPreviewRef}
        src={previewAudioSrc}
        onTimeUpdate={() => {
          if (audioPreviewRef.current) {
            setPlaybackCurrentTime(audioPreviewRef.current.currentTime);
          }
        }}
        onLoadedMetadata={() => {
          if (audioPreviewRef.current) {
            setPlaybackDuration(audioPreviewRef.current.duration || 0);
          }
        }}
        onEnded={() => setIsPlaying(false)}
        onError={() => setIsPlaying(false)}
        className="hidden"
      />

      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 bg-zinc-900/80 sticky top-0 z-10 backdrop-blur-xs">
        <div className="flex items-center space-x-2">
          <VolumeIcon size={16} className="text-indigo-400" />
          <h2 className="text-sm font-semibold text-zinc-100">Audio Mixdown & Ducking</h2>
        </div>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            className="text-zinc-400 hover:text-white p-1 rounded hover:bg-zinc-800 transition-colors"
            title="Close Mixer Panel"
          >
            ✕
          </button>
        )}
      </div>

      <div className="p-4 space-y-5">
        {/* Error banner */}
        {errorMessage && (
          <div className="p-2.5 rounded bg-red-950/40 border border-red-500/50 text-xs text-red-200 flex items-start justify-between">
            <span>{errorMessage}</span>
            <button
              type="button"
              onClick={() => setErrorMessage(null)}
              className="text-red-400 hover:text-white ml-2"
            >
              ✕
            </button>
          </div>
        )}

        {/* Mix Presets */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-zinc-200 flex items-center space-x-1.5">
              <SparklesIcon size={13} className="text-indigo-400" />
              <span>Mixing Preset</span>
            </span>
            <span className="text-[10px] text-zinc-400 font-mono">
              {config?.preset ? config.preset.toUpperCase() : 'CUSTOM'}
            </span>
          </div>

          <div className="grid grid-cols-3 gap-1.5 pt-1">
            <button
              type="button"
              onClick={() => handlePresetChange('balanced')}
              className={`px-2 py-1.5 rounded text-xs font-medium transition-colors border ${
                config?.preset === 'balanced'
                  ? 'bg-indigo-600 border-indigo-500 text-white shadow-xs'
                  : 'bg-zinc-800 border-zinc-700/60 text-zinc-300 hover:bg-zinc-700'
              }`}
            >
              Balanced
            </button>
            <button
              type="button"
              onClick={() => handlePresetChange('voiceFocus')}
              className={`px-2 py-1.5 rounded text-xs font-medium transition-colors border ${
                config?.preset === 'voiceFocus'
                  ? 'bg-indigo-600 border-indigo-500 text-white shadow-xs'
                  : 'bg-zinc-800 border-zinc-700/60 text-zinc-300 hover:bg-zinc-700'
              }`}
            >
              Voice Focus
            </button>
            <button
              type="button"
              onClick={() => handlePresetChange('preserveOriginal')}
              className={`px-2 py-1.5 rounded text-xs font-medium transition-colors border ${
                config?.preset === 'preserveOriginal'
                  ? 'bg-indigo-600 border-indigo-500 text-white shadow-xs'
                  : 'bg-zinc-800 border-zinc-700/60 text-zinc-300 hover:bg-zinc-700'
              }`}
            >
              Preserve Orig
            </button>
          </div>
        </div>

        {/* Master Gain Controls */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-4">
          <div className="text-xs font-semibold text-zinc-200 uppercase tracking-wider">
            Soundtrack & Voice Levels
          </div>

          {/* Original Soundtrack Gain */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <label className="flex items-center space-x-2 text-zinc-300">
                <input
                  type="checkbox"
                  checked={config?.originalAudioEnabled ?? true}
                  onChange={(e) => handleUpdateConfig({ originalAudioEnabled: e.target.checked })}
                  className="rounded border-zinc-700 bg-zinc-800 text-indigo-600 focus:ring-0"
                />
                <span className="font-medium">Original Video Soundtrack</span>
              </label>
              <span className="font-mono text-zinc-400">
                {config ? `${config.originalAudioGainDb.toFixed(1)} dB` : '0.0 dB'}
              </span>
            </div>
            <input
              type="range"
              min="-40"
              max="6"
              step="0.5"
              disabled={!config?.originalAudioEnabled}
              value={config?.originalAudioGainDb ?? -6.0}
              onChange={(e) => handleUpdateConfig({ originalAudioGainDb: parseFloat(e.target.value) })}
              className="w-full accent-indigo-500 cursor-pointer disabled:opacity-40"
            />
          </div>

          {/* Dubbing Master Voice Gain */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <span className="font-medium text-zinc-300">Dubbing Master Voice Gain</span>
              <span className="font-mono text-zinc-400">
                {config ? `${config.dubbingMasterGainDb.toFixed(1)} dB` : '0.0 dB'}
              </span>
            </div>
            <input
              type="range"
              min="-30"
              max="6"
              step="0.5"
              value={config?.dubbingMasterGainDb ?? 0.0}
              onChange={(e) => handleUpdateConfig({ dubbingMasterGainDb: parseFloat(e.target.value) })}
              className="w-full accent-indigo-500 cursor-pointer"
            />
          </div>
        </div>

        {/* Dialogue Ducking Engine */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-3">
          <div className="flex items-center justify-between">
            <label className="flex items-center space-x-2 text-xs font-semibold text-zinc-200">
              <input
                type="checkbox"
                checked={config?.duckingEnabled ?? true}
                onChange={(e) => handleUpdateConfig({ duckingEnabled: e.target.checked })}
                className="rounded border-zinc-700 bg-zinc-800 text-indigo-600 focus:ring-0"
              />
              <span>Dialogue Ducking</span>
            </label>
            <span className="text-[10px] text-zinc-400">
              {config?.duckingEnabled ? 'Active during speech' : 'Disabled'}
            </span>
          </div>

          {config?.duckingEnabled && (
            <div className="space-y-3 pt-1">
              {/* Attenuation */}
              <div className="space-y-1">
                <div className="flex items-center justify-between text-xs text-zinc-300">
                  <span>Ducking Attenuation</span>
                  <span className="font-mono text-zinc-400">{config.duckingAttenuationDb.toFixed(1)} dB</span>
                </div>
                <input
                  type="range"
                  min="-30"
                  max="-3"
                  step="0.5"
                  value={config.duckingAttenuationDb}
                  onChange={(e) => handleUpdateConfig({ duckingAttenuationDb: parseFloat(e.target.value) })}
                  className="w-full accent-indigo-500 cursor-pointer"
                />
              </div>

              {/* Envelope parameters: Attack, Hold, Release */}
              <div className="grid grid-cols-3 gap-2 pt-1 text-xs">
                <div>
                  <label className="block text-[10px] text-zinc-400 mb-1">Attack (ms)</label>
                  <input
                    type="number"
                    min="10"
                    max="300"
                    step="10"
                    value={config.duckingAttackMs}
                    onChange={(e) => handleUpdateConfig({ duckingAttackMs: parseInt(e.target.value) || 50 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-zinc-200 text-xs font-mono"
                  />
                </div>
                <div>
                  <label className="block text-[10px] text-zinc-400 mb-1">Hold (ms)</label>
                  <input
                    type="number"
                    min="0"
                    max="500"
                    step="20"
                    value={config.duckingHoldMs}
                    onChange={(e) => handleUpdateConfig({ duckingHoldMs: parseInt(e.target.value) || 100 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-zinc-200 text-xs font-mono"
                  />
                </div>
                <div>
                  <label className="block text-[10px] text-zinc-400 mb-1">Release (ms)</label>
                  <input
                    type="number"
                    min="50"
                    max="1000"
                    step="50"
                    value={config.duckingReleaseMs}
                    onChange={(e) => handleUpdateConfig({ duckingReleaseMs: parseInt(e.target.value) || 250 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-zinc-200 text-xs font-mono"
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Per-Track Volume & Solo/Mute */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-2.5">
          <div className="text-xs font-semibold text-zinc-200 uppercase tracking-wider">
            Track Audio Channels
          </div>

          <div className="space-y-2">
            {project.tracks.map((track) => {
              const settings = config?.trackSettings[track.id] || {
                trackId: track.id,
                isEnabled: track.isEnabled,
                isMuted: track.isMuted,
                isSolo: track.isSolo,
                gainDb: 0.0,
                pan: 0.0,
              };

              return (
                <div
                  key={track.id}
                  className="bg-zinc-800/50 border border-zinc-750 rounded p-2 text-xs space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium text-zinc-200 truncate">{track.displayName}</span>
                    <div className="flex items-center space-x-1.5">
                      <button
                        type="button"
                        onClick={() => handleTrackSettingChange(track.id, 'isMuted', !settings.isMuted)}
                        className={`px-1.5 py-0.5 rounded text-[10px] font-bold ${
                          settings.isMuted ? 'bg-amber-600 text-white' : 'bg-zinc-700 text-zinc-300'
                        }`}
                      >
                        M
                      </button>
                      <button
                        type="button"
                        onClick={() => handleTrackSettingChange(track.id, 'isSolo', !settings.isSolo)}
                        className={`px-1.5 py-0.5 rounded text-[10px] font-bold ${
                          settings.isSolo ? 'bg-indigo-600 text-white' : 'bg-zinc-700 text-zinc-300'
                        }`}
                      >
                        S
                      </button>
                    </div>
                  </div>

                  <div className="flex items-center space-x-2">
                    <input
                      type="range"
                      min="-20"
                      max="6"
                      step="0.5"
                      disabled={settings.isMuted}
                      value={settings.gainDb}
                      onChange={(e) =>
                        handleTrackSettingChange(track.id, 'gainDb', parseFloat(e.target.value))
                      }
                      className="flex-1 accent-indigo-500 cursor-pointer disabled:opacity-40"
                    />
                    <span className="font-mono text-[10px] text-zinc-400 w-12 text-right">
                      {settings.gainDb.toFixed(1)} dB
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Offline Mixdown Execution */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-zinc-200">Render Mixed WAV</span>
            <span
              className={`text-[10px] font-semibold px-2 py-0.5 rounded ${
                readiness?.isReady
                  ? 'bg-emerald-950/60 text-emerald-400 border border-emerald-500/30'
                  : 'bg-amber-950/60 text-amber-400 border border-amber-500/30'
              }`}
            >
              {readiness?.isReady ? 'Ready' : 'Not Ready'}
            </span>
          </div>

          {isRendering ? (
            <div className="space-y-2">
              <div className="w-full bg-zinc-800 rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-indigo-500 h-1.5 transition-all duration-300"
                  style={{ width: `${Math.round(renderProgress * 100)}%` }}
                />
              </div>
              <div className="flex items-center justify-between text-[11px] text-zinc-400">
                <span>{renderMessage}</span>
                <button
                  type="button"
                  onClick={handleCancelMixdown}
                  className="text-red-400 hover:text-red-300 font-medium"
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : (
            <Button
              size="sm"
              variant="primary"
              onClick={handleStartMixdown}
              disabled={!readiness?.isReady || isRendering}
              className="w-full bg-indigo-600 hover:bg-indigo-500 font-medium text-xs py-2 shadow-xs"
            >
              <SparklesIcon size={13} className="mr-1.5" />
              {mixResult ? 'Re-render Audio Mixdown' : 'Render Audio Mixdown'}
            </Button>
          )}

          {isMixStale && (
            <p className="text-[11px] text-amber-400 flex items-center space-x-1">
              <AlertTriangleIcon size={12} className="shrink-0" />
              <span>Settings changed since last render. Re-render to update mixdown.</span>
            </p>
          )}
        </div>

        {/* Genuine Audio A/B Preview Player */}
        <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-lg p-3 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-zinc-200">A/B Audio Audition</span>
            <div className="flex items-center space-x-1 bg-zinc-800 p-0.5 rounded text-[10px]">
              <button
                type="button"
                onClick={() => setPreviewTrack('A')}
                className={`px-2 py-0.5 rounded font-medium transition-colors ${
                  previewTrack === 'A' ? 'bg-indigo-600 text-white' : 'text-zinc-400 hover:text-zinc-200'
                }`}
              >
                A: Original
              </button>
              <button
                type="button"
                onClick={() => setPreviewTrack('B')}
                className={`px-2 py-0.5 rounded font-medium transition-colors ${
                  previewTrack === 'B' ? 'bg-indigo-600 text-white' : 'text-zinc-400 hover:text-zinc-200'
                }`}
              >
                B: Rendered Mix
              </button>
            </div>
          </div>

          {/* Transport Controls */}
          <div className="flex items-center space-x-2">
            <Button
              size="xs"
              variant="secondary"
              onClick={togglePlayPreview}
              disabled={!previewAudioSrc}
              className="px-3"
            >
              {isPlaying ? <PauseIcon size={12} /> : <PlayIcon size={12} />}
            </Button>

            <Button
              size="xs"
              variant="secondary"
              onClick={() => handleSeekPreview(0)}
              disabled={!previewAudioSrc}
              title="Return to start"
            >
              <UndoIcon size={12} />
            </Button>

            {/* Scrubber */}
            <input
              type="range"
              min="0"
              max={playbackDuration || 1}
              step="0.05"
              value={playbackCurrentTime}
              onChange={(e) => handleSeekPreview(parseFloat(e.target.value))}
              disabled={!previewAudioSrc}
              className="flex-1 accent-indigo-500 cursor-pointer disabled:opacity-40"
            />

            <span className="font-mono text-[10px] text-zinc-400 shrink-0">
              {playbackCurrentTime.toFixed(1)}s / {playbackDuration.toFixed(1)}s
            </span>
          </div>

          {/* Acoustic Loudness & Peak Analysis Results */}
          {mixResult && previewTrack === 'B' && (
            <div className="pt-2 border-t border-zinc-800/80 space-y-1.5 text-[11px]">
              <div className="flex items-center justify-between text-zinc-400">
                <span>Sample Peak:</span>
                <span
                  className={`font-mono font-medium ${
                    mixResult.peakDbfs > -0.5 ? 'text-red-400' : 'text-emerald-400'
                  }`}
                >
                  {mixResult.peakDbfs.toFixed(1)} dBFS
                </span>
              </div>
              <div className="flex items-center justify-between text-zinc-400">
                <span>RMS Level:</span>
                <span className="font-mono text-zinc-300">{mixResult.rmsDbfs.toFixed(1)} dBFS</span>
              </div>
              {mixResult.integratedLufs !== undefined && (
                <div className="flex items-center justify-between text-zinc-400">
                  <span>Integrated Loudness:</span>
                  <span className="font-mono text-zinc-300">{mixResult.integratedLufs.toFixed(1)} LUFS</span>
                </div>
              )}
              <div className="flex items-center justify-between text-zinc-400">
                <span>Clipping Samples:</span>
                <span
                  className={`font-mono ${
                    mixResult.clippingSamples > 0 ? 'text-red-400 font-bold' : 'text-emerald-400'
                  }`}
                >
                  {mixResult.clippingSamples}
                </span>
              </div>
              <div className="flex items-center justify-between text-zinc-400">
                <span>PCM Format:</span>
                <span className="font-mono text-zinc-300">
                  {mixResult.sampleRate} Hz · {mixResult.channels === 1 ? 'Mono' : 'Stereo'} · {mixResult.bitDepth}-bit
                </span>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
