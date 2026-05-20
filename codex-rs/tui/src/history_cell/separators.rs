//! Turn separators and runtime-metrics labels for transcript history.

use super::*;
use codex_config::types::TuiTiming;
use codex_config::types::TuiTimingMetric;
use codex_config::types::TuiTimingSymbols;

#[derive(Debug)]
/// A visual divider between turns, optionally showing how long the assistant "worked for".
///
/// This separator is only emitted for turns that performed concrete work (e.g., running commands,
/// applying patches, making MCP tool calls), so purely conversational turns do not show an empty
/// divider.
pub struct FinalMessageSeparator {
    elapsed_seconds: Option<u64>,
    runtime_metrics: Option<RuntimeMetricsSummary>,
}
impl FinalMessageSeparator {
    /// Creates a separator; completed turns should pass protocol turn duration when available.
    pub(crate) fn new(
        elapsed_seconds: Option<u64>,
        runtime_metrics: Option<RuntimeMetricsSummary>,
    ) -> Self {
        Self {
            elapsed_seconds,
            runtime_metrics,
        }
    }
}
impl HistoryCell for FinalMessageSeparator {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut label_parts = Vec::new();
        if let Some(elapsed_seconds) = self
            .elapsed_seconds
            .filter(|seconds| *seconds > 60)
            .map(crate::status_indicator_widget::fmt_elapsed_compact)
        {
            label_parts.push(format!("Worked for {elapsed_seconds}"));
        }
        if let Some(metrics_label) = self.runtime_metrics.and_then(runtime_metrics_label) {
            label_parts.push(metrics_label);
        }

        if label_parts.is_empty() {
            return vec![Line::from_iter(["─".repeat(width as usize).dim()])];
        }

        let label = format!("─ {} ─", label_parts.join(" • "));
        let (label, _suffix, label_width) = take_prefix_by_width(&label, width as usize);
        vec![
            Line::from_iter([
                label,
                "─".repeat((width as usize).saturating_sub(label_width)),
            ])
            .dim(),
        ]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut label_parts = Vec::new();
        if let Some(elapsed_seconds) = self
            .elapsed_seconds
            .filter(|seconds| *seconds > 60)
            .map(crate::status_indicator_widget::fmt_elapsed_compact)
        {
            label_parts.push(format!("Worked for {elapsed_seconds}"));
        }
        if let Some(metrics_label) = self.runtime_metrics.and_then(runtime_metrics_label) {
            label_parts.push(metrics_label);
        }
        if label_parts.is_empty() {
            Vec::new()
        } else {
            vec![Line::from(label_parts.join(" • "))]
        }
    }
}

pub(crate) fn runtime_metrics_label(summary: RuntimeMetricsSummary) -> Option<String> {
    let mut parts = Vec::new();
    if summary.tool_calls.count > 0 {
        let duration = format_duration_ms(summary.tool_calls.duration_ms);
        let calls = pluralize(summary.tool_calls.count, "call", "calls");
        parts.push(format!(
            "Local tools: {} {calls} ({duration})",
            summary.tool_calls.count
        ));
    }
    if summary.api_calls.count > 0 {
        let duration = format_duration_ms(summary.api_calls.duration_ms);
        let calls = pluralize(summary.api_calls.count, "call", "calls");
        parts.push(format!(
            "Inference: {} {calls} ({duration})",
            summary.api_calls.count
        ));
    }
    if summary.websocket_calls.count > 0 {
        let duration = format_duration_ms(summary.websocket_calls.duration_ms);
        parts.push(format!(
            "WebSocket: {} events send ({duration})",
            summary.websocket_calls.count
        ));
    }
    if summary.streaming_events.count > 0 {
        let duration = format_duration_ms(summary.streaming_events.duration_ms);
        let stream_label = pluralize(summary.streaming_events.count, "Stream", "Streams");
        let events = pluralize(summary.streaming_events.count, "event", "events");
        parts.push(format!(
            "{stream_label}: {} {events} ({duration})",
            summary.streaming_events.count
        ));
    }
    if summary.websocket_events.count > 0 {
        let duration = format_duration_ms(summary.websocket_events.duration_ms);
        parts.push(format!(
            "{} events received ({duration})",
            summary.websocket_events.count
        ));
    }
    if summary.responses_api_overhead_ms > 0 {
        let duration = format_duration_ms(summary.responses_api_overhead_ms);
        parts.push(format!("Responses API overhead: {duration}"));
    }
    if summary.responses_api_inference_time_ms > 0 {
        let duration = format_duration_ms(summary.responses_api_inference_time_ms);
        parts.push(format!("Responses API inference: {duration}"));
    }
    if summary.responses_api_engine_iapi_ttft_ms > 0
        || summary.responses_api_engine_service_ttft_ms > 0
    {
        let mut ttft_parts = Vec::new();
        if summary.responses_api_engine_iapi_ttft_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_iapi_ttft_ms);
            ttft_parts.push(format!("{duration} (iapi)"));
        }
        if summary.responses_api_engine_service_ttft_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_service_ttft_ms);
            ttft_parts.push(format!("{duration} (service)"));
        }
        parts.push(format!("TTFT: {}", ttft_parts.join(" ")));
    }
    if summary.responses_api_engine_iapi_tbt_ms > 0
        || summary.responses_api_engine_service_tbt_ms > 0
    {
        let mut tbt_parts = Vec::new();
        if summary.responses_api_engine_iapi_tbt_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_iapi_tbt_ms);
            tbt_parts.push(format!("{duration} (iapi)"));
        }
        if summary.responses_api_engine_service_tbt_ms > 0 {
            let duration = format_duration_ms(summary.responses_api_engine_service_tbt_ms);
            tbt_parts.push(format!("{duration} (service)"));
        }
        parts.push(format!("TBT: {}", tbt_parts.join(" ")));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" • "))
    }
}

pub(crate) fn compact_runtime_metrics_label(
    summary: RuntimeMetricsSummary,
    config: &TuiTiming,
) -> Option<String> {
    if !config.enabled {
        return None;
    }

    let metrics = config
        .show
        .clone()
        .unwrap_or_else(|| vec![TuiTimingMetric::Ttft, TuiTimingMetric::Tbt]);
    let symbols = config.symbols.as_ref();

    let mut parts = Vec::new();
    for metric in metrics {
        let part = match metric {
            TuiTimingMetric::Ttft => preferred_timing_metric(
                summary.responses_api_engine_service_ttft_ms,
                summary.responses_api_engine_iapi_ttft_ms,
                summary.turn_ttft_ms,
            )
            .map(|duration| {
                format!(
                    "{}{}",
                    metric_symbol(symbols, metric),
                    format_duration_ms(duration)
                )
            }),
            TuiTimingMetric::Tbt => preferred_timing_metric(
                summary.responses_api_engine_service_tbt_ms,
                summary.responses_api_engine_iapi_tbt_ms,
                summary.turn_ttfm_ms,
            )
            .map(|duration| {
                format!(
                    "{}{}",
                    metric_symbol(symbols, metric),
                    format_duration_ms(duration)
                )
            }),
            TuiTimingMetric::Model => (summary.responses_api_inference_time_ms > 0).then(|| {
                format!(
                    "{}{}",
                    metric_symbol(symbols, metric),
                    format_duration_ms(summary.responses_api_inference_time_ms)
                )
            }),
            TuiTimingMetric::Overhead => (summary.responses_api_overhead_ms > 0).then(|| {
                format!(
                    "{}{}",
                    metric_symbol(symbols, metric),
                    format_duration_ms(summary.responses_api_overhead_ms)
                )
            }),
        };
        if let Some(part) = part {
            parts.push(part);
        }
    }

    (!parts.is_empty()).then(|| parts.join("  "))
}

fn preferred_timing_metric(
    primary_ms: u64,
    fallback_ms: u64,
    turn_fallback_ms: u64,
) -> Option<u64> {
    if primary_ms > 0 {
        Some(primary_ms)
    } else if fallback_ms > 0 {
        Some(fallback_ms)
    } else if turn_fallback_ms > 0 {
        Some(turn_fallback_ms)
    } else {
        None
    }
}

fn metric_symbol(symbols: Option<&TuiTimingSymbols>, metric: TuiTimingMetric) -> &str {
    match metric {
        TuiTimingMetric::Ttft => symbols.and_then(|s| s.ttft.as_deref()).unwrap_or(""),
        TuiTimingMetric::Tbt => symbols.and_then(|s| s.tbt.as_deref()).unwrap_or("≋"),
        TuiTimingMetric::Model => symbols.and_then(|s| s.model.as_deref()).unwrap_or("◉"),
        TuiTimingMetric::Overhead => symbols.and_then(|s| s.overhead.as_deref()).unwrap_or("+"),
    }
}

fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms >= 1_000 {
        let seconds = duration_ms as f64 / 1_000.0;
        format!("{seconds:.1}s")
    } else {
        format!("{duration_ms}ms")
    }
}

fn pluralize(count: u64, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}
