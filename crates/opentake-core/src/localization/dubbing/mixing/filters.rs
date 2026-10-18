//! FFmpeg audio filtergraph generation for multitrack mixing, ducking, and limiting.

use super::models::{db_to_linear, DubbingMixPlan, DuckingInterval};

/// Builds the ducking volume expression for FFmpeg's `volume` filter.
///
/// Produces a piecewise continuous linear envelope with attack ramps, hold plateaus, and release ramps:
/// - Attack: smooth linear attenuation from unity (1.0) to attenuation (A)
/// - Hold: flat plateau at attenuation (A) during speech
/// - Release: smooth linear ramp from attenuation (A) back to unity (1.0)
pub fn build_ducking_volume_expression(intervals: &[DuckingInterval]) -> Option<String> {
    if intervals.is_empty() {
        return None;
    }

    let mut interval_exprs: Vec<String> = Vec::with_capacity(intervals.len());

    for interval in intervals {
        let att_linear = db_to_linear(interval.attenuation_db);
        let t0 = (interval.start_us.saturating_sub(interval.attack_us) as f64) / 1_000_000.0;
        let t1 = (interval.start_us as f64) / 1_000_000.0;
        let t2 = (interval.end_us as f64) / 1_000_000.0;
        let t3 = ((interval.end_us + interval.release_us) as f64) / 1_000_000.0;

        let att_dur = (t1 - t0).max(0.001);
        let rel_dur = (t3 - t2).max(0.001);

        // Piecewise envelope:
        // if between(t, t0, t1): 1.0 - (1.0 - A) * (t - t0) / att_dur
        // else if between(t, t1, t2): A
        // else if between(t, t2, t3): A + (1.0 - A) * (t - t2) / rel_dur
        // else: 1.0
        let expr = format!(
            "if(between(t,{t0:.3},{t1:.3}),1.0-(1.0-{att_linear:.4})*(t-{t0:.3})/{att_dur:.3},if(between(t,{t1:.3},{t2:.3}),{att_linear:.4},if(between(t,{t2:.3},{t3:.3}),{att_linear:.4}+(1.0-{att_linear:.4})*(t-{t2:.3})/{rel_dur:.3},1.0)))"
        );
        interval_exprs.push(expr);
    }

    // Combine multiple intervals using min(expr1, min(expr2, ...))
    if interval_exprs.len() == 1 {
        Some(interval_exprs.remove(0))
    } else {
        let mut combined = interval_exprs.pop().unwrap();
        while let Some(prev) = interval_exprs.pop() {
            combined = format!("min({prev},{combined})");
        }
        Some(combined)
    }
}

/// Generates the complete `-filter_complex` string for system FFmpeg.
pub fn build_complex_filter_graph(plan: &DubbingMixPlan) -> String {
    let mut filter_chains: Vec<String> = Vec::new();
    let mut mix_inputs: Vec<String> = Vec::new();
    let target_sr = plan.output_sample_rate;
    let layout_str = if plan.output_channels == 1 {
        "mono"
    } else {
        "stereo"
    };

    let mut input_idx = 0;

    // 1. Process original soundtrack if present
    if plan.has_original_audio {
        let mut orig_filters: Vec<String> = Vec::new();
        // Resample and conform layout
        orig_filters.push(format!("aresample={target_sr}"));
        orig_filters.push(format!(
            "aformat=channel_layouts={layout_str}:sample_fmts=s16"
        ));

        // Overall original audio gain
        let orig_gain_lin = db_to_linear(plan.original_gain_db);
        orig_filters.push(format!("volume={orig_gain_lin:.4}"));

        // Dialogue ducking envelope
        if let Some(duck_expr) = build_ducking_volume_expression(&plan.ducking_intervals) {
            orig_filters.push(format!("volume='{duck_expr}':eval=frame"));
        }

        let orig_chain = format!("[{input_idx}:a]{}[orig_mix]", orig_filters.join(","));
        filter_chains.push(orig_chain);
        mix_inputs.push("[orig_mix]".to_string());
        input_idx += 1;
    }

    // 2. Process each active speech segment
    for (seg_idx, seg) in plan.segments.iter().enumerate() {
        let mut seg_filters: Vec<String> = Vec::new();

        // If segment is NOT pre-aligned, trim according to source_in and source_out
        if !seg.is_aligned && (seg.source_in_us > 0 || seg.source_out_us > seg.source_in_us) {
            let in_pts = (seg.source_in_us as f64) / 1_000_000.0;
            let out_pts = (seg.source_out_us as f64) / 1_000_000.0;
            seg_filters.push(format!("atrim=start={in_pts:.4}:end={out_pts:.4}"));
            seg_filters.push("asetpts=PTS-STARTPTS".to_string());
        }

        // Resample and conform layout
        seg_filters.push(format!("aresample={target_sr}"));
        seg_filters.push(format!(
            "aformat=channel_layouts={layout_str}:sample_fmts=s16"
        ));

        // Apply segment gain + dubbing master gain
        let effective_gain_db = seg.gain_db + plan.dubbing_master_gain_db;
        let gain_lin = db_to_linear(effective_gain_db);
        seg_filters.push(format!("volume={gain_lin:.4}"));

        // Optional segment fades
        if seg.fade_in_ms > 0 {
            let fade_in_s = (seg.fade_in_ms as f64) / 1000.0;
            seg_filters.push(format!("afade=t=in:st=0:d={fade_in_s:.3}"));
        }
        if seg.fade_out_ms > 0 {
            let fade_out_s = (seg.fade_out_ms as f64) / 1000.0;
            let seg_dur_s = (seg.duration_us as f64) / 1_000_000.0;
            let start_out = (seg_dur_s - fade_out_s).max(0.0);
            seg_filters.push(format!("afade=t=out:st={start_out:.3}:d={fade_out_s:.3}"));
        }

        // Timeline delay in sample frames: (start_us * sample_rate) / 1_000_000
        let delay_samples = (seg.timeline_start_us * (target_sr as u64)) / 1_000_000;
        seg_filters.push(format!("adelay=delays={delay_samples}S:all=1"));

        let label = format!("[voice_{seg_idx}]");
        let seg_chain = format!("[{input_idx}:a]{}{label}", seg_filters.join(","));
        filter_chains.push(seg_chain);
        mix_inputs.push(label);
        input_idx += 1;
    }

    // 3. Final summation and peak limiter
    if mix_inputs.len() == 1 {
        // Only one audio stream, route directly through peak limiter
        let single_input = &mix_inputs[0];
        let final_chain = format!(
            "{single_input}alimiter=limit=0.891:attack=5:release=50:asc=1,aformat=sample_fmts=s16:sample_rates={target_sr}:channel_layouts={layout_str}[out]"
        );
        filter_chains.push(final_chain);
    } else if mix_inputs.len() > 1 {
        let inputs_str = mix_inputs.join("");
        let num_inputs = mix_inputs.len();
        let amix_chain = format!(
            "{inputs_str}amix=inputs={num_inputs}:normalize=0,alimiter=limit=0.891:attack=5:release=50:asc=1,aformat=sample_fmts=s16:sample_rates={target_sr}:channel_layouts={layout_str}[out]"
        );
        filter_chains.push(amix_chain);
    } else {
        // Fallback generator for completely silent project
        filter_chains.push(format!(
            "anullsrc=r={target_sr}:cl={layout_str}:d=1.0,aformat=sample_fmts=s16:sample_rates={target_sr}:channel_layouts={layout_str}[out]"
        ));
    }

    filter_chains.join(";")
}
