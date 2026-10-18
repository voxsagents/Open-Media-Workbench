import React, { useState } from "react";
import type { MediaAsset } from "../../types/media";
import type {
  SubtitleDocument,
  SubtitleFormat,
  SubtitleOperation,
  SubtitleTrack,
} from "../../types/subtitles";

import {
  SubtitlesIcon,
  GlobeIcon,
  MicIcon,
  UploadIcon,
  SparklesIcon,
  ShieldCheckIcon,
  DownloadIcon,
  CheckIcon,
  VolumeIcon,
} from "../../components/ui/Icons";
import { Button } from "../../components/ui/Button";
import { SubtitleTrackManager } from "./SubtitleTrackManager";
import { exportSubtitleTrack } from "../../lib/subtitles";
import { TranscriptionPanel } from "./TranscriptionPanel";
import { ModelManagerView } from "../../components/ai/ModelManagerView";
import type { AlignmentResult } from "../../types/alignment";
import { WordAlignmentPanel } from "./WordAlignmentPanel";
import { SubtitleStylingPanel } from "./SubtitleStylingPanel";
import { SubtitleBurnInPanel } from "./SubtitleBurnInPanel";
import { TranslationPanel } from "./TranslationPanel";
import { BilingualReviewPanel } from "./BilingualReviewPanel";
import { GlossaryManagerPanel } from "./GlossaryManagerPanel";
import { TextToSpeechPanel } from "./TextToSpeechPanel";
import { VoiceLibraryPanel } from "./VoiceLibraryPanel";
import { DubbingTimelinePanel } from "./DubbingTimelinePanel";
import type { SubtitleStyle } from "../../types/styling";
import { DEFAULT_SUBTITLE_STYLE } from "../../lib/styling";

export type LocalizationTab =
  | "subtitles"
  | "transcription"
  | "alignment"
  | "styling"
  | "translation"
  | "review"
  | "glossary"
  | "voices"
  | "dubbing"
  | "export";

export interface LocalizationInspectorProps {
  asset: MediaAsset | null;
  activeTab?: LocalizationTab;
  onTabChange?: (tab: LocalizationTab) => void;
  document: SubtitleDocument | null;
  activeTrack: SubtitleTrack | null;
  currentTime: number;
  selectedCueIds?: string[];
  onSelectCues?: (cueIds: string[]) => void;
  onApplyOperation?: (op: SubtitleOperation) => void;
  canUndo?: boolean;
  canRedo?: boolean;
  onUndo?: () => void;
  onRedo?: () => void;
  onDocumentChange: (doc: SubtitleDocument | null) => void;
  onActiveTrackChange: (trackId: string) => void;
  onToggleTrackVisibility: (trackId: string) => void;
  onSeek: (seconds: number) => void;
  alignmentResult?: AlignmentResult | null;
  enableKaraoke?: boolean;
  onToggleKaraoke?: () => void;
  showWordMarkers?: boolean;
  onToggleWordMarkers?: () => void;
  currentStyle?: SubtitleStyle;
  onStyleChange?: (style: SubtitleStyle) => void;
}

export function LocalizationInspector({
  asset,
  activeTab: controlledTab,
  onTabChange,
  document,
  activeTrack,
  currentTime,
  selectedCueIds,
  onSelectCues,
  onApplyOperation,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  onDocumentChange,
  onActiveTrackChange,
  onToggleTrackVisibility,
  onSeek,
  alignmentResult,
  enableKaraoke = true,
  onToggleKaraoke,
  showWordMarkers = true,
  onToggleWordMarkers,
  currentStyle = DEFAULT_SUBTITLE_STYLE,
  onStyleChange,
}: LocalizationInspectorProps): React.ReactElement {

  const [internalTab, setInternalTab] = useState<LocalizationTab>("subtitles");
  const activeTab = controlledTab !== undefined ? controlledTab : internalTab;
  const [showModelManager, setShowModelManager] = useState(false);

  const [exportFormat, setExportFormat] = useState<SubtitleFormat>("srt");
  const [exportFeedback, setExportFeedback] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [dubbingMode, setDubbingMode] = useState<"timeline" | "script">("timeline");

  const handleTabClick = (tab: LocalizationTab) => {
    setInternalTab(tab);
    onTabChange?.(tab);
  };

  const handleRunExport = async () => {
    if (!activeTrack) return;
    setIsExporting(true);
    setExportFeedback(null);

    // Optional review gate: verify if track has unresolved translation issues
    if (document?.metadata?.translationReview) {
      try {
        const rev = JSON.parse(document.metadata.translationReview);
        if (rev.targetTrackId === activeTrack.id) {
          const errorsCount = rev.entries.filter((e: { findings?: Array<{ severity: string }> }) =>
            e.findings?.some((f) => f.severity === "Error"),
          ).length;
          const unapprovedCount = rev.entries.filter((e: { status?: string }) => e.status !== "Approved").length;

          if (errorsCount > 0) {
            if (!confirm(`Quality Alert: This track contains ${errorsCount} unresolved terminology or quality error(s). Proceed with export anyway?`)) {
              setIsExporting(false);
              return;
            }
          } else if (unapprovedCount > 0) {
            if (!confirm(`Review Notice: ${unapprovedCount} cue(s) in this track have not yet been marked 'Approved'. Proceed with export?`)) {
              setIsExporting(false);
              return;
            }
          }
        }
      } catch {
        // Non-blocking metadata parse fail
      }
    }

    try {
      const res = await exportSubtitleTrack(activeTrack, exportFormat);
      if (res) {
        setExportFeedback(`Exported ${activeTrack.displayName} to ${res.exportedPath}`);
      }
    } catch (err) {
      setExportFeedback(`Export failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="h-full flex flex-col bg-zinc-950 text-zinc-100 select-none">
      {/* Tab Navigation Header */}
      <div
        role="tablist"
        aria-label="Localization Inspector Views"
        className="h-10 shrink-0 border-b border-zinc-800 bg-zinc-900/60 px-2 flex items-center space-x-1"
      >
        <button
          role="tab"
          id="loc-tab-subtitles"
          aria-selected={activeTab === "subtitles"}
          aria-controls="loc-panel-subtitles"
          tabIndex={activeTab === "subtitles" ? 0 : -1}
          onClick={() => handleTabClick("subtitles")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "subtitles"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <SubtitlesIcon size={13} className={activeTab === "subtitles" ? "text-indigo-400" : ""} />
          <span>Subtitles</span>
        </button>

        <button
          role="tab"
          id="loc-tab-transcription"
          aria-selected={activeTab === "transcription"}
          aria-controls="loc-panel-transcription"
          tabIndex={activeTab === "transcription" ? 0 : -1}
          onClick={() => handleTabClick("transcription")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "transcription"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <MicIcon size={13} className={activeTab === "transcription" ? "text-indigo-400" : ""} />
          <span>Transcribe</span>
        </button>

        <button
          role="tab"
          id="loc-tab-alignment"
          aria-selected={activeTab === "alignment"}
          aria-controls="loc-panel-alignment"
          tabIndex={activeTab === "alignment" ? 0 : -1}
          onClick={() => handleTabClick("alignment")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "alignment"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <SparklesIcon size={13} className={activeTab === "alignment" ? "text-indigo-400" : ""} />
          <span>Align</span>
        </button>

        <button
          role="tab"
          id="loc-tab-styling"
          aria-selected={activeTab === "styling"}
          aria-controls="loc-panel-styling"
          tabIndex={activeTab === "styling" ? 0 : -1}
          onClick={() => handleTabClick("styling")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "styling"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <SparklesIcon size={13} className={activeTab === "styling" ? "text-amber-400" : ""} />
          <span>Style</span>
        </button>

        <button
          role="tab"
          id="loc-tab-translation"
          aria-selected={activeTab === "translation"}
          aria-controls="loc-panel-translation"
          tabIndex={activeTab === "translation" ? 0 : -1}
          onClick={() => handleTabClick("translation")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "translation"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <GlobeIcon size={13} className={activeTab === "translation" ? "text-indigo-400" : ""} />
          <span>Translate</span>
        </button>

        <button
          role="tab"
          id="loc-tab-review"
          aria-selected={activeTab === "review"}
          aria-controls="loc-panel-review"
          tabIndex={activeTab === "review" ? 0 : -1}
          onClick={() => handleTabClick("review")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "review"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <ShieldCheckIcon size={13} className={activeTab === "review" ? "text-emerald-400" : ""} />
          <span>Review</span>
        </button>

        <button
          role="tab"
          id="loc-tab-glossary"
          aria-selected={activeTab === "glossary"}
          aria-controls="loc-panel-glossary"
          tabIndex={activeTab === "glossary" ? 0 : -1}
          onClick={() => handleTabClick("glossary")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "glossary"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <SparklesIcon size={13} className={activeTab === "glossary" ? "text-amber-400" : ""} />
          <span>Glossary</span>
        </button>

        <button
          role="tab"
          id="loc-tab-voices"
          aria-selected={activeTab === "voices"}
          aria-controls="loc-panel-voices"
          tabIndex={activeTab === "voices" ? 0 : -1}
          onClick={() => handleTabClick("voices")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "voices"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <VolumeIcon size={13} className={activeTab === "voices" ? "text-indigo-400" : ""} />
          <span>Voices</span>
        </button>

        <button
          role="tab"
          id="loc-tab-dubbing"
          aria-selected={activeTab === "dubbing"}
          aria-controls="loc-panel-dubbing"
          tabIndex={activeTab === "dubbing" ? 0 : -1}
          onClick={() => handleTabClick("dubbing")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "dubbing"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <MicIcon size={13} className={activeTab === "dubbing" ? "text-indigo-400" : ""} />
          <span>Speech</span>
        </button>

        <button
          role="tab"
          id="loc-tab-export"
          aria-selected={activeTab === "export"}
          aria-controls="loc-panel-export"
          tabIndex={activeTab === "export" ? 0 : -1}
          onClick={() => handleTabClick("export")}
          className={`flex items-center space-x-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 ${
            activeTab === "export"
              ? "bg-zinc-800 text-zinc-100 shadow-xs"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40"
          }`}
        >
          <UploadIcon size={13} className={activeTab === "export" ? "text-indigo-400" : ""} />
          <span>Export</span>
        </button>
      </div>

      {/* Tab Panels */}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {/* SUBTITLES PANEL */}
        {activeTab === "subtitles" && (
          <div id="loc-panel-subtitles" role="tabpanel" aria-labelledby="loc-tab-subtitles" className="h-full flex flex-col">
            <SubtitleTrackManager
              asset={asset}
              document={document}
              activeTrack={activeTrack}
              currentTime={currentTime}
              selectedCueIds={selectedCueIds}
              onSelectCues={onSelectCues}
              onApplyOperation={onApplyOperation}
              canUndo={canUndo}
              canRedo={canRedo}
              onUndo={onUndo}
              onRedo={onRedo}
              onDocumentChange={onDocumentChange}
              onActiveTrackChange={onActiveTrackChange}
              onToggleTrackVisibility={onToggleTrackVisibility}
              onSeek={onSeek}
              onOpenTranscription={() => handleTabClick("transcription")}
              onOpenTranslation={() => handleTabClick("translation")}
            />
          </div>
        )}

        {/* TRANSCRIPTION PANEL */}
        {activeTab === "transcription" && (
          <div id="loc-panel-transcription" role="tabpanel" aria-labelledby="loc-tab-transcription" className="h-full flex flex-col">
            <TranscriptionPanel
              asset={asset}
              document={document}
              onDocumentChange={(newDoc) => {
                onDocumentChange(newDoc);
                if (newDoc.activeTrackId) {
                  onActiveTrackChange(newDoc.activeTrackId);
                }
                handleTabClick("subtitles");
              }}
              onOpenModelManager={() => setShowModelManager(true)}
              onClose={() => handleTabClick("subtitles")}
            />
          </div>
        )}

        {/* WORD ALIGNMENT & SMART SEGMENTATION PANEL */}
        {activeTab === "alignment" && (
          <div id="loc-panel-alignment" role="tabpanel" aria-labelledby="loc-tab-alignment" className="h-full flex flex-col overflow-y-auto">
            <WordAlignmentPanel
              alignmentResult={alignmentResult || null}
              activeTrack={activeTrack}
              document={document}
              onApplyTrack={(newDoc) => {
                onDocumentChange(newDoc);
                if (newDoc.activeTrackId) {
                  onActiveTrackChange(newDoc.activeTrackId);
                }
                handleTabClick("subtitles");
              }}
              onSeek={onSeek}
              enableKaraoke={enableKaraoke}
              onToggleKaraoke={onToggleKaraoke || (() => {})}
              showWordMarkers={showWordMarkers}
              onToggleWordMarkers={onToggleWordMarkers || (() => {})}
              currentTimeUs={Math.round(currentTime * 1_000_000)}
            />
          </div>
        )}

        {/* STYLING PANEL (STEP 21) */}
        {activeTab === "styling" && (
          <div id="loc-panel-styling" role="tabpanel" aria-labelledby="loc-tab-styling" className="p-4 overflow-y-auto">
            <SubtitleStylingPanel
              currentStyle={currentStyle}
              onStyleChange={onStyleChange || (() => {})}
              onNavigateToExport={() => handleTabClick("export")}
            />
          </div>
        )}

        {/* TRANSLATION PANEL (STEP 22 Active) */}
        {activeTab === "translation" && (
          <div id="loc-panel-translation" role="tabpanel" aria-labelledby="loc-tab-translation" className="h-full overflow-hidden">
            <TranslationPanel
              document={document}
              onUpdateDocument={onDocumentChange}
              onOpenModelManager={() => setShowModelManager(true)}
            />
          </div>
        )}

        {/* BILINGUAL REVIEW PANEL (STEP 23 Active) */}
        {activeTab === "review" && (
          <div id="loc-panel-review" role="tabpanel" aria-labelledby="loc-tab-review" className="h-full overflow-hidden">
            <BilingualReviewPanel
              document={document}
              onDocumentChange={onDocumentChange}
              currentTime={currentTime}
              onSeek={onSeek}
              canUndo={canUndo}
              canRedo={canRedo}
              onUndo={onUndo}
              onRedo={onRedo}
            />
          </div>
        )}

        {/* GLOSSARY & TRANSLATION MEMORY PANEL (STEP 23 Active) */}
        {activeTab === "glossary" && (
          <div id="loc-panel-glossary" role="tabpanel" aria-labelledby="loc-tab-glossary" className="h-full overflow-hidden">
            <GlossaryManagerPanel />
          </div>
        )}

        {/* AI VOICE LIBRARY PANEL (STEP 25 Active) */}
        {activeTab === "voices" && (
          <div id="loc-panel-voices" role="tabpanel" aria-labelledby="loc-tab-voices" className="h-full overflow-hidden">
            <VoiceLibraryPanel
              document={document}
              activeTrack={activeTrack}
              onDocumentChange={(newDoc) => onDocumentChange(newDoc)}
              onOpenModelManager={() => setShowModelManager(true)}
              onSelectVoiceForTts={(_voiceId) => {
                handleTabClick("dubbing");
              }}
            />
          </div>
        )}

        {/* DUBBING & SPEECH PANEL (STEP 26 Active) */}
        {activeTab === "dubbing" && (
          <div id="loc-panel-dubbing" role="tabpanel" aria-labelledby="loc-tab-dubbing" className="h-full flex flex-col overflow-hidden">
            {/* Dubbing sub-mode toggle */}
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-zinc-800 bg-zinc-900/80 shrink-0">
              <span className="text-[11px] font-semibold text-zinc-300">AI Dubbing Studio</span>
              <div className="flex items-center space-x-1 bg-zinc-950 p-0.5 rounded border border-zinc-800">
                <button
                  type="button"
                  onClick={() => setDubbingMode("timeline")}
                  className={`px-2 py-0.5 text-[10px] font-medium rounded transition-colors ${
                    dubbingMode === "timeline"
                      ? "bg-indigo-600 text-white shadow-xs"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  Dubbing Timeline
                </button>
                <button
                  type="button"
                  onClick={() => setDubbingMode("script")}
                  className={`px-2 py-0.5 text-[10px] font-medium rounded transition-colors ${
                    dubbingMode === "script"
                      ? "bg-indigo-600 text-white shadow-xs"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  Script TTS
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-hidden">
              {dubbingMode === "timeline" ? (
                <DubbingTimelinePanel
                  document={document}
                  activeTrack={activeTrack}
                  asset={asset}
                  currentTime={currentTime}
                  onSeek={onSeek}
                  onDocumentChange={onDocumentChange}
                  onOpenVoiceLibrary={() => handleTabClick("voices")}
                  onOpenModelManager={() => setShowModelManager(true)}
                />
              ) : (
                <TextToSpeechPanel
                  document={document}
                  activeTrack={activeTrack}
                  onOpenModelManager={() => setShowModelManager(true)}
                  onOpenVoiceLibrary={() => handleTabClick("voices")}
                  onSeek={onSeek}
                />
              )}
            </div>
          </div>
        )}

        {/* EXPORT PANEL */}
        {activeTab === "export" && (
          <div id="loc-panel-export" role="tabpanel" aria-labelledby="loc-tab-export" className="p-4 space-y-4 overflow-y-auto">
            <div className="flex items-center justify-between">
              <h4 className="text-xs font-semibold text-zinc-200 flex items-center space-x-1.5">
                <UploadIcon size={14} className="text-indigo-400" />
                <span>Localization Export</span>
              </h4>
              <span className="px-1.5 py-0.2 rounded text-[10px] bg-emerald-950 text-emerald-300 border border-emerald-800">
                STEP 16 Active
              </span>
            </div>

            {/* Subtitle Export Section */}
            <div className="p-3.5 rounded-lg border border-zinc-800 bg-zinc-900/60 space-y-3">
              <div className="flex items-center justify-between">
                <label className="text-xs font-semibold text-zinc-200 block">
                  Export Subtitle File (.srt / .vtt)
                </label>
                <span className="text-[10px] text-zinc-400 font-mono">
                  {activeTrack ? `${activeTrack.displayName} (${activeTrack.cues.length} cues)` : "No Active Track"}
                </span>
              </div>

              <div className="space-y-2">
                <label className="text-[11px] text-zinc-400 block">Export Format</label>
                <div className="flex items-center space-x-2">
                  <label className="flex items-center space-x-1.5 text-xs text-zinc-300 cursor-pointer">
                    <input
                      type="radio"
                      name="exportFormat"
                      checked={exportFormat === "srt"}
                      onChange={() => setExportFormat("srt")}
                      className="text-indigo-600 focus:ring-indigo-500"
                    />
                    <span>SubRip (.srt)</span>
                  </label>
                  <label className="flex items-center space-x-1.5 text-xs text-zinc-300 cursor-pointer ml-3">
                    <input
                      type="radio"
                      name="exportFormat"
                      checked={exportFormat === "webVtt"}
                      onChange={() => setExportFormat("webVtt")}
                      className="text-indigo-600 focus:ring-indigo-500"
                    />
                    <span>WebVTT (.vtt)</span>
                  </label>
                </div>
              </div>

              <Button
                size="xs"
                variant="primary"
                onClick={handleRunExport}
                disabled={!activeTrack || isExporting}
                className="w-full bg-indigo-600 hover:bg-indigo-500 text-white font-medium"
              >
                <DownloadIcon size={12} className="mr-1.5" />
                {activeTrack
                  ? `Export ${activeTrack.displayName} as ${exportFormat.toUpperCase()}`
                  : "Select a track to export"}
              </Button>

              {exportFeedback && (
                <div className="text-[11px] p-2 rounded bg-zinc-950 border border-zinc-800 text-zinc-300 flex items-center space-x-1.5">
                  <CheckIcon size={12} className="text-indigo-400 shrink-0" />
                  <span className="truncate">{exportFeedback}</span>
                </div>
              )}
            </div>

            {/* Subtitle Burn-In Video Export Section (STEP 21 Active) */}
            <SubtitleBurnInPanel
              asset={asset}
              activeTrack={activeTrack}
              currentStyle={currentStyle}
              onNavigateToStyling={() => handleTabClick("styling")}
            />

            {/* Future Audio Muxing Capability Notice */}
            <div className="p-3.5 rounded-lg border border-zinc-800/80 bg-zinc-900/30 space-y-2.5">
              <div className="text-xs font-semibold text-zinc-400">Advanced Audio Muxing (Scheduled)</div>
              <div className="space-y-2 opacity-60">
                <label className="flex items-center space-x-2 text-xs text-zinc-400 cursor-not-allowed">
                  <input type="checkbox" disabled className="rounded bg-zinc-800 border-zinc-700" />
                  <span>Multiplex Dubbed Audio Stream (Scheduled STEP 30)</span>
                </label>
              </div>
            </div>
          </div>
        )}
      </div>

      {showModelManager && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4">
          <div className="bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl w-full max-w-4xl max-h-[85vh] flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 bg-zinc-950/50">
              <span className="text-sm font-semibold text-zinc-200">AI Model Manager</span>
              <button
                onClick={() => setShowModelManager(false)}
                className="text-zinc-400 hover:text-zinc-200 text-xs px-2 py-1 rounded bg-zinc-800 hover:bg-zinc-700"
              >
                Close
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4">
              <ModelManagerView />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
