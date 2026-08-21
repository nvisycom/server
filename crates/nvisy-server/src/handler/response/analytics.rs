//! Workspace analytics response types.
//!
//! Breakdowns are ordered arrays of labeled entries (not maps) so a client can
//! feed them straight to a chart, and every enum value is present — zero-filled
//! when it has no data — so a series never has a missing bar.

use std::collections::BTreeMap;

use jiff::ToSpan;
use jiff::civil::Date;
use nvisy_postgres::query::{AnalyticsSnapshot, RunDayPoint};
use nvisy_postgres::types::{FileKind, PipelineRunStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

/// Aggregate analytics for a workspace: what it stores, how its runs fare, and
/// the inference tokens they spent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAnalytics {
    /// Stored-file totals and their per-kind breakdown.
    pub storage: StorageAnalytics,
    /// Pipeline-run health: volume, status mix, and durations.
    pub runs: RunAnalytics,
    /// Inference token usage: workspace totals and a per-model breakdown.
    pub usage: UsageAnalytics,
}

/// Inference token usage across a workspace's runs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalytics {
    /// Total input/prompt tokens across all models.
    pub input_tokens: i64,
    /// Total output/completion tokens across all models.
    pub output_tokens: i64,
    /// Total tokens as reported across all models (not necessarily input +
    /// output).
    pub total_tokens: i64,
    /// Per-model breakdown, one entry per model used, in a stable order.
    pub by_model: Vec<ModelUsageEntry>,
}

/// One model's token usage across a workspace's runs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageEntry {
    /// The model.
    pub model: String,
    /// Input/prompt tokens summed for this model (`0` if never reported).
    pub input_tokens: i64,
    /// Output/completion tokens summed for this model (`0` if never reported).
    pub output_tokens: i64,
    /// Reported total tokens summed for this model (`0` if never reported).
    pub total_tokens: i64,
}

/// Storage totals across a workspace's live files.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageAnalytics {
    /// Total bytes of all live files.
    pub total_bytes: i64,
    /// Number of live files.
    pub file_count: i64,
    /// Per-kind breakdown, one entry per `file_kind` (zero-filled), in a stable
    /// order.
    pub by_kind: Vec<StorageKindEntry>,
}

/// One `file_kind`'s share of a workspace's storage.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageKindEntry {
    /// The file kind.
    pub kind: FileKind,
    /// Number of live files of this kind.
    pub file_count: i64,
    /// Total bytes of live files of this kind.
    pub total_bytes: i64,
}

/// Pipeline-run health for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunAnalytics {
    /// Total number of runs.
    pub total: i64,
    /// Per-status breakdown, one entry per run status (zero-filled), in a stable
    /// order.
    pub by_status: Vec<RunStatusEntry>,
    /// Failed / (completed + failed). Omitted when no run has reached a terminal
    /// state (genuinely no signal, not zero).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// Mean completed-run duration in milliseconds; omitted until a run completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_duration_ms: Option<i64>,
    /// 95th-percentile completed-run duration in milliseconds; omitted until a run
    /// completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_duration_ms: Option<i64>,
}

/// One status's share of a workspace's runs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusEntry {
    /// The run status.
    pub status: PipelineRunStatus,
    /// Number of runs in this status.
    pub count: i64,
}

impl WorkspaceAnalytics {
    /// Assembles the response from a workspace analytics snapshot, zero-filling
    /// every enum value and deriving the scalar totals and error rate.
    pub fn from_snapshot(snapshot: AnalyticsSnapshot) -> Self {
        let AnalyticsSnapshot {
            storage,
            runs,
            durations,
            usage,
        } = snapshot;
        let by_kind: Vec<StorageKindEntry> = FileKind::iter()
            .map(|kind| {
                let row = storage.iter().find(|r| r.file_kind == kind);
                StorageKindEntry {
                    kind,
                    file_count: row.map_or(0, |r| r.file_count),
                    total_bytes: row.map_or(0, |r| r.total_bytes),
                }
            })
            .collect();
        let total_bytes = by_kind.iter().map(|e| e.total_bytes).sum();
        let file_count = by_kind.iter().map(|e| e.file_count).sum();

        let by_status: Vec<RunStatusEntry> = PipelineRunStatus::iter()
            .map(|status| RunStatusEntry {
                status,
                count: runs
                    .iter()
                    .find(|r| r.status == status)
                    .map_or(0, |r| r.count),
            })
            .collect();
        let total = by_status.iter().map(|e| e.count).sum();

        let completed = count_of(&by_status, PipelineRunStatus::Completed);
        let failed = count_of(&by_status, PipelineRunStatus::Failed);
        let terminal = completed + failed;
        let error_rate = (terminal > 0).then(|| failed as f64 / terminal as f64);

        let by_model: Vec<ModelUsageEntry> = usage
            .into_iter()
            .map(|u| ModelUsageEntry {
                model: u.model,
                input_tokens: u.input_tokens.unwrap_or(0),
                output_tokens: u.output_tokens.unwrap_or(0),
                total_tokens: u.total_tokens.unwrap_or(0),
            })
            .collect();
        let usage = UsageAnalytics {
            input_tokens: by_model.iter().map(|m| m.input_tokens).sum(),
            output_tokens: by_model.iter().map(|m| m.output_tokens).sum(),
            total_tokens: by_model.iter().map(|m| m.total_tokens).sum(),
            by_model,
        };

        Self {
            storage: StorageAnalytics {
                total_bytes,
                file_count,
                by_kind,
            },
            runs: RunAnalytics {
                total,
                by_status,
                error_rate,
                avg_duration_ms: durations.avg_ms,
                p95_duration_ms: durations.p95_ms,
            },
            usage,
        }
    }
}

/// A workspace's daily run activity over a window: one point per day, dense
/// (quiet days included with `runs: 0`), ready to plot as a continuous series.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunTimeSeries {
    /// One entry per day in the requested window, oldest first.
    pub points: Vec<RunDayEntry>,
}

/// A single day of run activity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunDayEntry {
    /// The day (`YYYY-MM-DD`, UTC).
    pub date: Date,
    /// Runs started this day (`0` on a quiet day).
    pub runs: i64,
    /// Failed / (completed + failed) for this day; omitted when no run reached a
    /// terminal state that day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// Mean completed-run duration (milliseconds) this day; omitted if none completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_duration_ms: Option<i64>,
    /// 95th-percentile completed-run duration (milliseconds) this day; omitted if none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_duration_ms: Option<i64>,
    /// Input/prompt tokens spent by this day's runs; omitted when none used a model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    /// Output/completion tokens spent this day; omitted when none used a model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    /// Reported total tokens this day; omitted when none used a model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
}

impl RunTimeSeries {
    /// Builds a dense daily series over `[from, to]` from the sparse per-day rows
    /// the query returns. Every day in the window is emitted in order; a day with
    /// no runs reports `runs: 0` and omits the rate/duration/token fields. The
    /// query only produces days that had a run, so gap-filling happens here.
    pub fn from_window(from: Date, to: Date, points: Vec<RunDayPoint>) -> Self {
        // Index the sparse rows by their UTC day for O(1) lookup while walking the
        // window.
        let mut by_day: BTreeMap<Date, RunDayPoint> = BTreeMap::new();
        for p in points {
            let date = p.day.to_zoned(jiff::tz::TimeZone::UTC).date();
            by_day.insert(date, p);
        }

        let mut days = Vec::new();
        let mut date = from;
        while date <= to {
            days.push(match by_day.remove(&date) {
                Some(p) => RunDayEntry {
                    date,
                    runs: p.runs,
                    error_rate: (p.terminal > 0).then(|| p.failed as f64 / p.terminal as f64),
                    avg_duration_ms: p.avg_ms,
                    p95_duration_ms: p.p95_ms,
                    input_tokens: p.input_tokens,
                    output_tokens: p.output_tokens,
                    total_tokens: p.total_tokens,
                },
                None => RunDayEntry {
                    date,
                    runs: 0,
                    error_rate: None,
                    avg_duration_ms: None,
                    p95_duration_ms: None,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                },
            });
            let Ok(next) = date.checked_add(1.days()) else {
                break;
            };
            date = next;
        }

        Self { points: days }
    }
}

/// Looks up a status's count in the assembled breakdown.
fn count_of(by_status: &[RunStatusEntry], status: PipelineRunStatus) -> i64 {
    by_status
        .iter()
        .find(|e| e.status == status)
        .map_or(0, |e| e.count)
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::query::{RunDurations, RunStatusCount, StorageByKind, UsageByModel};

    use super::*;

    #[test]
    fn zero_fills_every_kind_and_status_and_derives_totals() {
        let storage = vec![
            StorageByKind {
                file_kind: FileKind::Original,
                file_count: 2,
                total_bytes: 300,
            },
            StorageByKind {
                file_kind: FileKind::Redacted,
                file_count: 1,
                total_bytes: 50,
            },
        ];
        let runs = vec![
            RunStatusCount {
                status: PipelineRunStatus::Completed,
                count: 3,
            },
            RunStatusCount {
                status: PipelineRunStatus::Failed,
                count: 1,
            },
        ];
        let durations = RunDurations {
            avg_ms: Some(30_000),
            p95_ms: Some(56_000),
        };

        let usage = vec![
            UsageByModel {
                model: "gpt-4".to_string(),
                input_tokens: Some(130),
                output_tokens: Some(60),
                total_tokens: None,
            },
            UsageByModel {
                model: "claude".to_string(),
                input_tokens: Some(200),
                output_tokens: None,
                total_tokens: Some(250),
            },
        ];
        let a = WorkspaceAnalytics::from_snapshot(AnalyticsSnapshot {
            storage,
            runs,
            durations,
            usage,
        });

        // Every FileKind / PipelineRunStatus is present (zero-filled), so the
        // breakdown lengths equal the enum sizes.
        assert_eq!(a.storage.by_kind.len(), FileKind::iter().count());
        assert_eq!(a.runs.by_status.len(), PipelineRunStatus::iter().count());
        // Absent audit kind is zero, not missing.
        let audit = a
            .storage
            .by_kind
            .iter()
            .find(|e| e.kind == FileKind::Audit)
            .unwrap();
        assert_eq!((audit.file_count, audit.total_bytes), (0, 0));

        // Totals sum the breakdown.
        assert_eq!(a.storage.total_bytes, 350);
        assert_eq!(a.storage.file_count, 3);
        assert_eq!(a.runs.total, 4);

        // error_rate = failed / (completed + failed) = 1 / 4.
        assert_eq!(a.runs.error_rate, Some(0.25));
        assert_eq!(a.runs.avg_duration_ms, Some(30_000));

        // Usage: per-model entries preserved, workspace totals summed with
        // never-reported fields treated as 0 (not conflated across fields).
        assert_eq!(a.usage.by_model.len(), 2);
        assert_eq!(a.usage.input_tokens, 330); // 130 + 200
        assert_eq!(a.usage.output_tokens, 60); // 60 + 0 (claude none)
        assert_eq!(a.usage.total_tokens, 250); // 0 + 250 (gpt-4 none)
    }

    #[test]
    fn timeseries_gap_fills_window_and_derives_per_day_error_rate() {
        let date = |s: &str| jiff::civil::Date::strptime("%Y-%m-%d", s).unwrap();
        let day_ts = |s: &str| {
            date(s)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
        };
        // Only the active day is returned by the query (sparse); Jan 6 and 7 have
        // no runs and must be gap-filled by from_window over the Jan 5..7 window.
        let sparse = vec![RunDayPoint {
            day: day_ts("2026-01-05"),
            runs: 3,
            terminal: 3,
            failed: 1,
            avg_ms: Some(20_000),
            p95_ms: Some(28_000),
            input_tokens: Some(1000),
            output_tokens: Some(400),
            total_tokens: None,
        }];

        let series = RunTimeSeries::from_window(date("2026-01-05"), date("2026-01-07"), sparse);

        // Dense: every day in the window present, in order.
        assert_eq!(series.points.len(), 3);
        assert_eq!(series.points[0].date, date("2026-01-05"));
        assert_eq!(series.points[2].date, date("2026-01-07"));

        // Active day: derived error rate + tokens carried through.
        let d5 = &series.points[0];
        assert_eq!(d5.runs, 3);
        assert_eq!(d5.error_rate, Some(1.0 / 3.0));
        assert_eq!(d5.avg_duration_ms, Some(20_000));
        assert_eq!(d5.input_tokens, Some(1000));
        assert_eq!(d5.total_tokens, None);

        // Gap-filled days: runs 0, everything else omitted.
        let d6 = &series.points[1];
        assert_eq!(d6.runs, 0);
        assert_eq!(d6.error_rate, None);
        assert_eq!(d6.avg_duration_ms, None);
        assert_eq!(d6.input_tokens, None);
    }

    #[test]
    fn error_rate_is_none_with_no_terminal_runs() {
        // Only active runs, no files, no completed durations.
        let runs = vec![RunStatusCount {
            status: PipelineRunStatus::Queued,
            count: 5,
        }];
        let a = WorkspaceAnalytics::from_snapshot(AnalyticsSnapshot {
            storage: vec![],
            runs,
            durations: RunDurations {
                avg_ms: None,
                p95_ms: None,
            },
            usage: Vec::new(),
        });

        assert_eq!(a.runs.error_rate, None);
        assert_eq!(a.runs.avg_duration_ms, None);
        assert_eq!(a.storage.total_bytes, 0);
        // Still every kind/status present, all zero except queued.
        assert_eq!(a.storage.by_kind.len(), FileKind::iter().count());
        assert_eq!(a.runs.total, 5);
    }
}
