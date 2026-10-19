//! Workspace analytics response types.
//!
//! Breakdowns are ordered arrays of labeled entries (not maps) so a client can
//! feed them straight to a chart, and every enum value is present — zero-filled
//! when it has no data — so a series never has a missing bar.

use std::collections::{BTreeMap, HashMap};

use jiff::ToSpan;
use jiff::civil::Date;
use nvisy_postgres::query::{AnalyticsSnapshot, DetectionDayPoint};
use nvisy_postgres::types::{DetectionStatus, DocumentKind, UsageMeter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

/// Converts a per-meter map into its API form: keyed by the meter's camelCase
/// name (`tokensInput`, `seconds`, …), carrying only the meters present.
fn meter_map(meters: &HashMap<UsageMeter, f64>) -> BTreeMap<String, f64> {
    meters
        .iter()
        .map(|(meter, amount)| (meter_key(*meter).to_owned(), *amount))
        .collect()
}

/// The camelCase API key for a meter.
fn meter_key(meter: UsageMeter) -> &'static str {
    match meter {
        UsageMeter::TokensInput => "tokensInput",
        UsageMeter::TokensOutput => "tokensOutput",
        UsageMeter::TokensTotal => "tokensTotal",
        UsageMeter::Seconds => "seconds",
        UsageMeter::Pages => "pages",
        UsageMeter::Images => "images",
        UsageMeter::Characters => "characters",
    }
}

/// Aggregate analytics for a workspace: what it stores, how its detections fare,
/// and the inference usage they metered.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAnalytics {
    /// Stored-file totals and their per-kind breakdown.
    pub storage: WorkspaceStorageAnalytics,
    /// Detection health: volume, status mix, and durations.
    pub detections: WorkspaceDetectionAnalytics,
    /// Inference usage: per-meter workspace totals and a per-model breakdown.
    pub usage: WorkspaceUsageAnalytics,
}

/// Inference usage across a workspace's detections.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUsageAnalytics {
    /// Workspace-wide total per meter, keyed by meter name (`tokensInput`,
    /// `seconds`, …). Only meters actually reported appear.
    pub meters: BTreeMap<String, f64>,
    /// Total model calls across all detections.
    pub calls: i64,
    /// How many of those calls failed.
    pub errors: i64,
    /// Per-model breakdown, one entry per model used, in a stable order.
    pub by_model: Vec<WorkspaceModelUsageEntry>,
}

/// One model's usage across a workspace's detections.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceModelUsageEntry {
    /// The model.
    pub model: String,
    /// Summed amount per meter for this model, keyed by meter name. Only meters
    /// this model reported appear; a value may be fractional (seconds).
    pub meters: BTreeMap<String, f64>,
    /// Calls made to this model.
    pub calls: i64,
    /// How many of those calls failed.
    pub errors: i64,
}

/// Storage totals across a workspace's live files.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageAnalytics {
    /// Total bytes of all live files.
    pub total_bytes: i64,
    /// Number of live files.
    pub file_count: i64,
    /// Per-kind breakdown, one entry per document `kind` (zero-filled), in a
    /// stable order.
    pub by_kind: Vec<WorkspaceStorageKindEntry>,
}

/// One document `kind`'s share of a workspace's storage.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageKindEntry {
    /// The document kind.
    pub kind: DocumentKind,
    /// Number of live documents of this kind.
    pub file_count: i64,
    /// Total bytes of live documents of this kind.
    pub total_bytes: i64,
}

/// Detection health for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionAnalytics {
    /// Total number of detections.
    pub total: i64,
    /// Per-status breakdown, one entry per detection status (zero-filled), in a
    /// stable order.
    pub by_status: Vec<WorkspaceDetectionStatusEntry>,
    /// Failed / (completed + failed). Omitted when no detection has reached a
    /// terminal state (genuinely no signal, not zero).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// Mean completed-detection duration in milliseconds; omitted until a
    /// detection completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_duration_ms: Option<i64>,
    /// 95th-percentile completed-detection duration in milliseconds; omitted until
    /// a detection completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_duration_ms: Option<i64>,
}

/// One status's share of a workspace's detections.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionStatusEntry {
    /// The detection status.
    pub status: DetectionStatus,
    /// Number of detections in this status.
    pub count: i64,
}

impl WorkspaceAnalytics {
    /// Assembles the response from a workspace analytics snapshot, zero-filling
    /// every enum value and deriving the scalar totals and error rate.
    // reason: an error rate is inherently floating-point; the operands are small detection counts.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn from_snapshot(snapshot: AnalyticsSnapshot) -> Self {
        let AnalyticsSnapshot {
            storage,
            detections,
            durations,
            usage,
        } = snapshot;
        let by_kind: Vec<WorkspaceStorageKindEntry> = DocumentKind::iter()
            .map(|kind| {
                let row = storage.iter().find(|r| r.kind == kind);
                WorkspaceStorageKindEntry {
                    kind,
                    file_count: row.map_or(0, |r| r.file_count),
                    total_bytes: row.map_or(0, |r| r.total_bytes),
                }
            })
            .collect();
        let total_bytes = by_kind.iter().map(|e| e.total_bytes).sum();
        let file_count = by_kind.iter().map(|e| e.file_count).sum();

        let by_status: Vec<WorkspaceDetectionStatusEntry> = DetectionStatus::iter()
            .map(|status| WorkspaceDetectionStatusEntry {
                status,
                count: detections
                    .iter()
                    .find(|r| r.status == status)
                    .map_or(0, |r| r.count),
            })
            .collect();
        let total = by_status.iter().map(|e| e.count).sum();

        let completed = count_of(&by_status, DetectionStatus::Complete);
        let failed = count_of(&by_status, DetectionStatus::Failed);
        let terminal = completed + failed;
        let error_rate = (terminal > 0).then(|| failed as f64 / terminal as f64);

        let by_model: Vec<WorkspaceModelUsageEntry> = usage
            .into_iter()
            .map(|u| WorkspaceModelUsageEntry {
                model: u.model,
                meters: meter_map(&u.meters),
                calls: u.calls,
                errors: u.errors,
            })
            .collect();
        // Workspace totals: sum each meter and the call/error counts across models.
        let mut meters: BTreeMap<String, f64> = BTreeMap::new();
        for entry in &by_model {
            for (name, amount) in &entry.meters {
                *meters.entry(name.clone()).or_default() += amount;
            }
        }
        let usage = WorkspaceUsageAnalytics {
            meters,
            calls: by_model.iter().map(|m| m.calls).sum(),
            errors: by_model.iter().map(|m| m.errors).sum(),
            by_model,
        };

        Self {
            storage: WorkspaceStorageAnalytics {
                total_bytes,
                file_count,
                by_kind,
            },
            detections: WorkspaceDetectionAnalytics {
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

/// A workspace's daily detection activity over a window: one point per day, dense
/// (quiet days included with `detections: 0`), ready to plot as a continuous
/// series.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionTimeSeries {
    /// One entry per day in the requested window, oldest first.
    pub points: Vec<WorkspaceDetectionDayEntry>,
}

/// A single day of detection activity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionDayEntry {
    /// The day (`YYYY-MM-DD`, UTC).
    pub date: Date,
    /// Detections started this day (`0` on a quiet day).
    pub detections: i64,
    /// Failed / (completed + failed) for this day; omitted when no detection
    /// reached a terminal state that day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// Mean completed-detection duration (milliseconds) this day; omitted if none completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_duration_ms: Option<i64>,
    /// 95th-percentile completed-detection duration (milliseconds) this day; omitted if none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_duration_ms: Option<i64>,
    /// Amount per meter spent by this day's detections, keyed by meter name.
    /// Empty on a day with no metered usage.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub meters: BTreeMap<String, f64>,
}

impl WorkspaceDetectionTimeSeries {
    /// Builds a dense daily series over `[from, to]` from the sparse per-day rows
    /// the query returns. Every day in the window is emitted in order; a day with
    /// no detections reports `detections: 0` and omits the rate/duration/token
    /// fields. The query only produces days that had a detection, so gap-filling
    /// happens here.
    // reason: an error rate is inherently floating-point; the operands are small detection counts.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn from_window(from: Date, to: Date, points: Vec<DetectionDayPoint>) -> Self {
        // Index the sparse rows by their UTC day for O(1) lookup while walking the
        // window.
        let mut by_day: BTreeMap<Date, DetectionDayPoint> = BTreeMap::new();
        for p in points {
            let date = p.day.to_zoned(jiff::tz::TimeZone::UTC).date();
            by_day.insert(date, p);
        }

        let mut days = Vec::new();
        let mut date = from;
        while date <= to {
            days.push(match by_day.remove(&date) {
                Some(p) => WorkspaceDetectionDayEntry {
                    date,
                    detections: p.detections,
                    error_rate: (p.terminal > 0).then(|| p.failed as f64 / p.terminal as f64),
                    avg_duration_ms: p.avg_ms,
                    p95_duration_ms: p.p95_ms,
                    meters: meter_map(&p.meters),
                },
                None => WorkspaceDetectionDayEntry {
                    date,
                    detections: 0,
                    error_rate: None,
                    avg_duration_ms: None,
                    p95_duration_ms: None,
                    meters: BTreeMap::new(),
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
fn count_of(by_status: &[WorkspaceDetectionStatusEntry], status: DetectionStatus) -> i64 {
    by_status
        .iter()
        .find(|e| e.status == status)
        .map_or(0, |e| e.count)
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::query::{
        DetectionDurations, DetectionStatusCount, StorageByKind, UsageByModel,
    };

    use super::*;

    #[test]
    fn zero_fills_every_kind_and_status_and_derives_totals() {
        // Only original documents have storage here; redacted is absent so the
        // zero-fill can be checked on a genuinely-missing kind.
        let storage = vec![StorageByKind {
            kind: DocumentKind::Original,
            file_count: 2,
            total_bytes: 300,
        }];
        let detections = vec![
            DetectionStatusCount {
                status: DetectionStatus::Complete,
                count: 3,
            },
            DetectionStatusCount {
                status: DetectionStatus::Failed,
                count: 1,
            },
        ];
        let durations = DetectionDurations {
            avg_ms: Some(30_000),
            p95_ms: Some(56_000),
        };

        let usage = vec![
            UsageByModel {
                model: "gpt-4".to_string(),
                meters: HashMap::from([
                    (UsageMeter::TokensInput, 130.0),
                    (UsageMeter::TokensOutput, 60.0),
                ]),
                calls: 2,
                errors: 0,
            },
            UsageByModel {
                model: "claude".to_string(),
                meters: HashMap::from([
                    (UsageMeter::TokensInput, 200.0),
                    (UsageMeter::TokensTotal, 250.0),
                ]),
                calls: 1,
                errors: 1,
            },
        ];
        let a = WorkspaceAnalytics::from_snapshot(AnalyticsSnapshot {
            storage,
            detections,
            durations,
            usage,
        });

        // Every DocumentKind / DetectionStatus is present (zero-filled), so the
        // breakdown lengths equal the enum sizes.
        assert_eq!(a.storage.by_kind.len(), DocumentKind::iter().count());
        assert_eq!(
            a.detections.by_status.len(),
            DetectionStatus::iter().count()
        );
        // Absent redacted kind is zero, not missing.
        let redacted = a
            .storage
            .by_kind
            .iter()
            .find(|e| e.kind == DocumentKind::Redacted)
            .unwrap();
        assert_eq!((redacted.file_count, redacted.total_bytes), (0, 0));

        // Totals sum the breakdown.
        assert_eq!(a.storage.total_bytes, 300);
        assert_eq!(a.storage.file_count, 2);
        assert_eq!(a.detections.total, 4);

        // error_rate = failed / (completed + failed) = 1 / 4.
        assert_eq!(a.detections.error_rate, Some(0.25));
        assert_eq!(a.detections.avg_duration_ms, Some(30_000));

        // Usage: per-model entries preserved; workspace meter totals sum each
        // meter across models (only reported meters appear), and call/error
        // counts sum too.
        assert_eq!(a.usage.by_model.len(), 2);
        assert_eq!(a.usage.meters.get("tokensInput"), Some(&330.0)); // 130 + 200
        assert_eq!(a.usage.meters.get("tokensOutput"), Some(&60.0)); // 60 (gpt-4 only)
        assert_eq!(a.usage.meters.get("tokensTotal"), Some(&250.0)); // 250 (claude only)
        assert_eq!(a.usage.calls, 3);
        assert_eq!(a.usage.errors, 1);
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
        // no detections and must be gap-filled by from_window over the Jan 5..7 window.
        let sparse = vec![DetectionDayPoint {
            day: day_ts("2026-01-05"),
            detections: 3,
            terminal: 3,
            failed: 1,
            avg_ms: Some(20_000),
            p95_ms: Some(28_000),
            meters: HashMap::from([
                (UsageMeter::TokensInput, 1000.0),
                (UsageMeter::TokensOutput, 400.0),
            ]),
        }];

        let series = WorkspaceDetectionTimeSeries::from_window(
            date("2026-01-05"),
            date("2026-01-07"),
            sparse,
        );

        // Dense: every day in the window present, in order.
        assert_eq!(series.points.len(), 3);
        assert_eq!(series.points[0].date, date("2026-01-05"));
        assert_eq!(series.points[2].date, date("2026-01-07"));

        // Active day: derived error rate + meters carried through.
        let d5 = &series.points[0];
        assert_eq!(d5.detections, 3);
        assert_eq!(d5.error_rate, Some(1.0 / 3.0));
        assert_eq!(d5.avg_duration_ms, Some(20_000));
        assert_eq!(d5.meters.get("tokensInput"), Some(&1000.0));
        assert!(!d5.meters.contains_key("tokensTotal"));

        // Gap-filled days: detections 0, everything else omitted (empty meters).
        let d6 = &series.points[1];
        assert_eq!(d6.detections, 0);
        assert_eq!(d6.error_rate, None);
        assert_eq!(d6.avg_duration_ms, None);
        assert!(d6.meters.is_empty());
    }

    #[test]
    fn error_rate_is_none_with_no_terminal_detections() {
        // Only in-progress detections, no files, no completed durations.
        let detections = vec![DetectionStatusCount {
            status: DetectionStatus::Pending,
            count: 5,
        }];
        let a = WorkspaceAnalytics::from_snapshot(AnalyticsSnapshot {
            storage: vec![],
            detections,
            durations: DetectionDurations {
                avg_ms: None,
                p95_ms: None,
            },
            usage: Vec::new(),
        });

        assert_eq!(a.detections.error_rate, None);
        assert_eq!(a.detections.avg_duration_ms, None);
        assert_eq!(a.storage.total_bytes, 0);
        // Still every kind/status present, all zero except queued.
        assert_eq!(a.storage.by_kind.len(), DocumentKind::iter().count());
        assert_eq!(a.detections.total, 5);
    }
}
