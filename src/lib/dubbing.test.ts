import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createDubbingProject,
  generateDubbingSegments,
  applyDubbingOperation,
  undoDubbingOperation,
  redoDubbingOperation,
  getDubbingDiagnostics,
  getDubbingWaveform,
  saveDubbingToDocument,
  loadDubbingFromDocument,
} from './dubbing';
import type { SubtitleDocument } from '../types/subtitles';

function createSampleDoc(): SubtitleDocument {
  return {
    schemaVersion: 1,
    id: 'doc-test-1',
    associatedMediaAssetId: 'video-asset-1',
    createdAtEpochMs: Date.now(),
    updatedAtEpochMs: Date.now(),
    tracks: [
      {
        id: 'sub-track-1',
        language: 'en',
        displayName: 'English Dialogue',
        sourceFormat: 'srt',
        isEnabled: true,
        isVisible: true,
        schemaVersion: 1,
        cues: [
          {
            id: 'cue-1',
            startUs: 1_000_000,
            endUs: 3_000_000,
            text: 'Welcome to Open Media Workbench.',
            customMetadata: {},
          },
          {
            id: 'cue-2',
            startUs: 4_000_000,
            endUs: 6_000_000,
            text: 'Create subtitles and translate videos.',
            customMetadata: {},
          },
        ],
      },
    ],
    metadata: {},
  };
}

describe('Dubbing Timeline Client & Domain Logic', () => {
  it('creates a dubbing project from a subtitle track', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    assert.ok(project);
    assert.equal(project.sourceProjectId, 'doc-test-1');
    assert.equal(project.tracks.length, 1);
    assert.equal(project.tracks[0].sourceSubtitleTrackId, 'sub-track-1');
    assert.equal(project.tracks[0].language, 'en');
  });

  it('synthesizes speech segments and places them at cue start times', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;

    const updated = await generateDubbingSegments(dubTrackId, sampleDoc);
    assert.equal(updated.tracks[0].segments.length, 2);

    const seg1 = updated.tracks[0].segments[0];
    const seg2 = updated.tracks[0].segments[1];
    assert.equal(seg1.timelineStartUs, 1_000_000);
    assert.equal(seg2.timelineStartUs, 4_000_000);
    assert.ok(seg1.audioAssetId);
    assert.ok(updated.assets[seg1.audioAssetId]);
  });

  it('performs non-destructive move, trim and split', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const seg1Id = project.tracks[0].segments[0].id;

    // Move
    const moved = await applyDubbingOperation({
      type: 'moveSegment',
      segmentId: seg1Id,
      newTimelineStartUs: 2_500_000,
    });
    assert.equal(moved.tracks[0].segments[0].timelineStartUs, 2_500_000);

    // Trim Start
    const trimmed = await applyDubbingOperation({
      type: 'trimSegmentStart',
      segmentId: seg1Id,
      newSourceInUs: 500_000,
    });
    assert.equal(trimmed.tracks[0].segments[0].sourceInUs, 500_000);
    assert.equal(trimmed.tracks[0].segments[0].timelineStartUs, 3_000_000);

    // Split
    const split = await applyDubbingOperation({
      type: 'splitSegment',
      segmentId: seg1Id,
      splitTimelineUs: 3_500_000,
    });
    assert.equal(split.tracks[0].segments.length, 3);
  });

  it('supports undo and redo editing history', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const seg1Id = project.tracks[0].segments[0].id;
    const initialStart = project.tracks[0].segments[0].timelineStartUs;

    // Move segment
    await applyDubbingOperation({
      type: 'moveSegment',
      segmentId: seg1Id,
      newTimelineStartUs: 5_000_000,
    });

    // Undo
    const undone = await undoDubbingOperation();
    const undoneSeg = undone?.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.equal(undoneSeg?.timelineStartUs, initialStart);

    // Redo
    const redone = await redoDubbingOperation();
    const redoneSeg = redone?.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.equal(redoneSeg?.timelineStartUs, 5_000_000);
  });

  it('detects timeline diagnostics and duration overruns', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const findings = await getDubbingDiagnostics(sampleDoc);
    assert.ok(Array.isArray(findings));
  });

  it('retrieves waveform peak summary', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const seg1Id = project.tracks[0].segments[0].id;
    const wf = await getDubbingWaveform(seg1Id);
    assert.equal(wf.segmentId, seg1Id);
    assert.ok(wf.bins.length > 0);
    assert.ok(wf.bins[0].max >= wf.bins[0].min);
  });

  it('persists project into SubtitleDocument metadata and reloads it', async () => {
    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const docWithDubbing = await saveDubbingToDocument(sampleDoc);
    assert.ok(docWithDubbing.metadata && docWithDubbing.metadata['dubbingProject']);

    const reloaded = await loadDubbingFromDocument(docWithDubbing);
    assert.ok(reloaded);
    assert.equal(reloaded?.id, project.id);
    assert.equal(reloaded?.tracks[0].segments.length, 2);
  });

  it('plans, processes, applies, and restores duration alignment', async () => {
    const {
      planDubbingAlignment,
      processDubbingAlignment,
      applyDubbingAlignment,
      restoreOriginalDubbingAudio,
      setDubbingAlignmentMode,
      setDubbingStartOffset,
      getDubbingAlignmentDiagnostics,
      batchAlignDubbingSegments,
    } = await import('./dubbing');

    const sampleDoc = createSampleDoc();
    const project = await createDubbingProject(sampleDoc, 'sub-track-1', 'en');
    const dubTrackId = project.tracks[0].id;
    await generateDubbingSegments(dubTrackId, sampleDoc);

    const seg1Id = project.tracks[0].segments[0].id;

    // 1. Plan alignment
    const plan = await planDubbingAlignment(seg1Id);
    assert.ok(plan);
    assert.equal(plan.segmentId, seg1Id);
    assert.ok(plan.proposedTempo > 0);

    // 2. Process alignment
    const [result, alignedAsset] = await processDubbingAlignment(plan);
    assert.ok(result);
    assert.ok(alignedAsset);
    assert.equal(result.status, 'aligned');

    // 3. Apply alignment
    const updated = await applyDubbingAlignment(seg1Id, plan, result, alignedAsset);
    const seg = updated.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.ok(seg);
    assert.equal(seg?.audioAssetId, alignedAsset.assetId);
    assert.equal(seg?.alignedAudioAssetId, alignedAsset.assetId);
    assert.equal(seg?.alignmentMode, 'autoFit');

    // 4. Set start offset
    const withOffset = await setDubbingStartOffset(seg1Id, 250_000);
    const segOffset = withOffset.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.equal(segOffset?.startOffsetUs, 250_000);

    // 5. Switch to original mode
    const switched = await setDubbingAlignmentMode(seg1Id, 'original');
    const segOrig = switched.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.equal(segOrig?.alignmentMode, 'original');

    // 6. Restore original audio completely
    const restored = await restoreOriginalDubbingAudio(seg1Id);
    const segRestored = restored.tracks[0].segments.find((s) => s.id === seg1Id);
    assert.equal(segRestored?.alignmentMode, 'original');

    // 7. Diagnostics
    const diags = await getDubbingAlignmentDiagnostics();
    assert.ok(Array.isArray(diags));

    // 8. Batch align
    const batchResult = await batchAlignDubbingSegments();
    assert.ok(batchResult);
  });
});
