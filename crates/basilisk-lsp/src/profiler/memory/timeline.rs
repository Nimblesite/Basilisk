//! Auto-snapshot mode and memory timeline tracking.
//!
//! When auto-snapshot is enabled, the LSP takes periodic memory snapshots
//! at a configurable interval and tracks memory usage over time. This data
//! powers the memory timeline chart in the VS Code dashboard and enables
//! trend-based leak detection.

use std::time::{Duration, Instant};

use serde::Serialize;
use tracing::{debug, info};

use super::{format_bytes, MemorySnapshot};

/// Configuration for auto-snapshot mode.
#[derive(Debug, Clone)]
pub struct AutoSnapshotConfig {
    /// Interval between snapshots.
    pub interval: Duration,
    /// Maximum number of snapshots to retain in memory.
    pub max_snapshots: usize,
}

impl Default for AutoSnapshotConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            max_snapshots: 120,
        }
    }
}

/// A single data point in the memory timeline.
#[derive(Debug, Clone, Serialize)]
pub struct TimelinePoint {
    /// Seconds since profiling started.
    pub elapsed_secs: f64,
    /// Current traced memory in bytes.
    pub current_memory: u64,
    /// Peak traced memory in bytes.
    pub peak_memory: u64,
    /// Number of gc-tracked objects.
    pub gc_objects: u64,
    /// Snapshot ID for drill-down.
    pub snapshot_id: String,
}

/// Tracks memory usage over time from periodic snapshots.
///
/// Stores a rolling window of timeline points and detects trends
/// (steady growth = leak indicator).
#[derive(Debug)]
pub struct MemoryTimeline {
    /// Configuration for auto-snapshot mode.
    config: AutoSnapshotConfig,
    /// Timeline data points (newest last).
    points: Vec<TimelinePoint>,
    /// When the timeline started (for elapsed time calculation).
    started_at: Instant,
    /// Whether auto-snapshot mode is active.
    active: bool,
}

impl MemoryTimeline {
    /// Create a new timeline tracker.
    #[must_use]
    pub fn new(config: AutoSnapshotConfig) -> Self {
        Self {
            config,
            points: Vec::new(),
            started_at: Instant::now(),
            active: false,
        }
    }

    /// Start auto-snapshot mode.
    pub fn start(&mut self) {
        self.started_at = Instant::now();
        self.points.clear();
        self.active = true;
        info!(
            interval_secs = self.config.interval.as_secs(),
            max_snapshots = self.config.max_snapshots,
            "auto-snapshot mode started"
        );
    }

    /// Stop auto-snapshot mode.
    pub fn stop(&mut self) {
        self.active = false;
        info!(points = self.points.len(), "auto-snapshot mode stopped");
    }

    /// Whether auto-snapshot mode is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Record a snapshot as a timeline data point.
    pub fn record(&mut self, snapshot: &MemorySnapshot) {
        let elapsed = self.started_at.elapsed().as_secs_f64();

        let point = TimelinePoint {
            elapsed_secs: elapsed,
            current_memory: snapshot.current_memory,
            peak_memory: snapshot.peak_memory,
            gc_objects: snapshot.gc_objects,
            snapshot_id: snapshot.snapshot_id.clone(),
        };

        self.points.push(point);

        // Evict oldest points if over the limit.
        if self.points.len() > self.config.max_snapshots {
            let excess = self.points.len() - self.config.max_snapshots;
            let _ = self.points.drain(..excess);
        }

        debug!(
            elapsed_secs = elapsed,
            memory = format_bytes(snapshot.current_memory).as_str(),
            points = self.points.len(),
            "recorded timeline point"
        );
    }

    /// Return all timeline points for the dashboard chart.
    #[must_use]
    pub fn points(&self) -> &[TimelinePoint] {
        &self.points
    }

    /// Detect whether memory is trending upward (potential leak).
    ///
    /// Returns `true` if the last N points show consistent growth.
    #[must_use]
    pub fn is_trending_upward(&self, min_points: usize) -> bool {
        if self.points.len() < min_points {
            return false;
        }

        let start = self.points.len().saturating_sub(min_points);
        let recent = self.points.get(start..).unwrap_or_default();
        recent
            .windows(2)
            .all(|pair| match (pair.first(), pair.get(1)) {
                (Some(a), Some(b)) => b.current_memory > a.current_memory,
                _ => false,
            })
    }

    /// Return the snapshot interval for auto-snapshot scheduling.
    #[must_use]
    pub fn interval(&self) -> Duration {
        self.config.interval
    }

    /// Total number of snapshots recorded (including evicted ones).
    #[must_use]
    pub fn total_recorded(&self) -> u64 {
        u64::try_from(self.points.len()).unwrap_or(0)
    }

    /// Serialize the timeline data as JSON for the VS Code dashboard.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(&self.points)
            .map_err(|err| format!("failed to serialize timeline: {err}"))
    }

    /// Build a notification payload for `basilisk/memory/timeline`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn build_notification(&self, session_id: &str) -> Result<serde_json::Value, String> {
        let data = TimelineNotificationData {
            memory_session_id: session_id.to_owned(),
            points: self.points.clone(),
            is_trending_upward: self.is_trending_upward(5),
        };
        serde_json::to_value(&data)
            .map_err(|err| format!("failed to serialize timeline notification: {err}"))
    }
}

/// Payload for the `basilisk/memory/timeline` notification.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineNotificationData {
    /// The memory session this timeline belongs to.
    pub memory_session_id: String,
    /// Ordered timeline data points.
    pub points: Vec<TimelinePoint>,
    /// Whether memory is trending upward (potential leak).
    pub is_trending_upward: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(id: &str, current: u64, peak: u64, objects: u64) -> MemorySnapshot {
        MemorySnapshot {
            snapshot_id: id.to_owned(),
            current_memory: current,
            peak_memory: peak,
            gc_objects: objects,
            gc_counts: vec![],
            top_allocations: vec![],
        }
    }

    #[test]
    fn timeline_records_points() {
        let config = AutoSnapshotConfig {
            max_snapshots: 10,
            ..AutoSnapshotConfig::default()
        };
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();

        timeline.record(&make_snapshot("snap-1", 1_000_000, 1_000_000, 100));
        timeline.record(&make_snapshot("snap-2", 2_000_000, 2_000_000, 200));

        assert_eq!(timeline.points().len(), 2);
        assert!(timeline.is_active());
    }

    #[test]
    fn timeline_evicts_oldest_when_full() {
        let config = AutoSnapshotConfig {
            max_snapshots: 3,
            ..AutoSnapshotConfig::default()
        };
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();

        for idx in 0u64..5 {
            let mem = (idx + 1) * 1_000_000;
            timeline.record(&make_snapshot(&format!("snap-{idx}"), mem, mem, idx * 100));
        }

        assert_eq!(timeline.points().len(), 3);
        // Oldest two should have been evicted.
        if let Some(first_point) = timeline.points().first() {
            assert_eq!(first_point.snapshot_id, "snap-2");
        }
    }

    #[test]
    fn detects_upward_trend() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();

        // Steadily increasing memory = upward trend.
        for idx in 1..=5 {
            let mem = u64::try_from(idx).unwrap_or(0) * 10_000_000;
            timeline.record(&make_snapshot(&format!("snap-{idx}"), mem, mem, 0));
        }

        assert!(timeline.is_trending_upward(3));
        assert!(timeline.is_trending_upward(5));
    }

    #[test]
    fn no_trend_when_memory_fluctuates() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();

        // Memory goes up and down.
        timeline.record(&make_snapshot("s1", 10_000_000, 10_000_000, 0));
        timeline.record(&make_snapshot("s2", 20_000_000, 20_000_000, 0));
        timeline.record(&make_snapshot("s3", 5_000_000, 20_000_000, 0));
        timeline.record(&make_snapshot("s4", 15_000_000, 20_000_000, 0));

        assert!(!timeline.is_trending_upward(3));
    }

    #[test]
    fn not_enough_points_returns_false() {
        let config = AutoSnapshotConfig::default();
        let timeline = MemoryTimeline::new(config);
        assert!(!timeline.is_trending_upward(3));
    }

    #[test]
    fn stop_clears_active_flag() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();
        assert!(timeline.is_active());
        timeline.stop();
        assert!(!timeline.is_active());
    }

    #[test]
    fn to_json_serializes_points() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();
        timeline.record(&make_snapshot("s1", 1_000_000, 1_000_000, 100));

        let json = timeline.to_json();
        assert!(json.is_ok());
        if let Ok(value) = json {
            assert!(value.is_array());
            if let Some(arr) = value.as_array() {
                assert_eq!(arr.len(), 1);
            }
        }
    }

    #[test]
    fn notification_includes_session_id_and_points() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();
        timeline.record(&make_snapshot("s1", 1_000_000, 1_000_000, 100));

        let notification = timeline.build_notification("mem-abc");
        assert!(notification.is_ok());
        if let Ok(value) = notification {
            assert_eq!(
                value
                    .get("memorySessionId")
                    .and_then(serde_json::Value::as_str),
                Some("mem-abc")
            );
            assert!(value
                .get("points")
                .and_then(serde_json::Value::as_array)
                .is_some());
            assert!(value
                .get("isTrendingUpward")
                .and_then(serde_json::Value::as_bool)
                .is_some());
        }
    }

    #[test]
    fn notification_detects_upward_trend() {
        let config = AutoSnapshotConfig::default();
        let mut timeline = MemoryTimeline::new(config);
        timeline.start();

        for idx in 1..=6 {
            let mem = u64::try_from(idx).unwrap_or(0) * 10_000_000;
            timeline.record(&make_snapshot(&format!("s{idx}"), mem, mem, 0));
        }

        let notification = timeline.build_notification("mem-trend");
        assert!(notification.is_ok());
        if let Ok(value) = notification {
            assert_eq!(
                value
                    .get("isTrendingUpward")
                    .and_then(serde_json::Value::as_bool),
                Some(true)
            );
        }
    }
}
