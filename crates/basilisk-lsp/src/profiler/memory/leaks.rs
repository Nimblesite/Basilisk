//! Memory leak confidence scoring.
//!
//! Analyzes patterns across multiple snapshot diffs to classify suspected
//! leaks by confidence level: Definite, High, Medium, Low.

use serde::Serialize;

use super::diff::AllocationGrowth;

/// Confidence level for a suspected memory leak.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum LeakConfidence {
    /// Single-diff growth, small size — could be normal warmup.
    Low,
    /// Growth in 2 consecutive diffs, or large single-diff growth (>10 MB).
    Medium,
    /// Consistent growth across 3+ consecutive diffs, no corresponding frees.
    High,
    /// Object has `__del__` and is in a reference cycle (uncollectable by gc).
    Definite,
}

impl std::fmt::Display for LeakConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Definite => write!(f, "DEFINITE"),
        }
    }
}

/// A suspected memory leak with confidence scoring.
#[derive(Debug, Clone, Serialize)]
pub struct SuspectedLeak {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
    /// Size growth in bytes.
    pub size_growth: i64,
    /// Object count growth.
    pub count_growth: i64,
    /// Current total size in bytes.
    pub current_size: u64,
    /// Current total object count.
    pub current_count: u64,
    /// Leak confidence level.
    pub confidence: LeakConfidence,
    /// Human-readable reason for the confidence level.
    pub reason: String,
    /// Allocation traceback.
    pub traceback: Vec<super::diff::TraceFrame>,
}

/// History of a single allocation site across multiple diffs.
#[derive(Debug, Clone, Default)]
struct AllocationHistory {
    /// Number of consecutive diffs where this site grew.
    consecutive_growths: u32,
    /// Total bytes grown across all observed diffs.
    total_growth: i64,
}

/// Tracks allocation patterns across multiple diffs for leak scoring.
#[derive(Debug, Default)]
pub struct LeakTracker {
    /// `(file, line)` -> growth history.
    histories: std::collections::HashMap<(String, i32), AllocationHistory>,
}

impl LeakTracker {
    /// Create a new leak tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a new set of growing allocations from a snapshot diff.
    ///
    /// Updates internal history and returns scored suspected leaks.
    pub fn process_growths(&mut self, growths: &[AllocationGrowth]) -> Vec<SuspectedLeak> {
        // Track which sites appeared in this diff.
        let mut seen = std::collections::HashSet::new();

        for growth in growths {
            let key = (growth.file.clone(), growth.line);
            let _ = seen.insert(key.clone());

            let history = self.histories.entry(key).or_default();
            history.consecutive_growths += 1;
            history.total_growth += growth.size_diff;
        }

        // Reset consecutive count for sites that didn't grow this time.
        let all_keys: Vec<_> = self.histories.keys().cloned().collect();
        for key in all_keys {
            if !seen.contains(&key) {
                if let Some(history) = self.histories.get_mut(&key) {
                    history.consecutive_growths = 0;
                }
            }
        }

        // Score each growth.
        growths
            .iter()
            .map(|growth| {
                let key = (growth.file.clone(), growth.line);
                let history = self.histories.get(&key).cloned().unwrap_or_default();
                let (confidence, reason) = score_leak(&history, growth);

                SuspectedLeak {
                    file: growth.file.clone(),
                    line: growth.line,
                    size_growth: growth.size_diff,
                    count_growth: growth.count_diff,
                    current_size: growth.size,
                    current_count: growth.count,
                    confidence,
                    reason,
                    traceback: growth.traceback.clone(),
                }
            })
            .collect()
    }
}

/// Large single-diff threshold: 10 MB.
const LARGE_GROWTH_THRESHOLD: i64 = 10 * 1024 * 1024;

/// Score a single allocation site based on its history and current growth.
fn score_leak(history: &AllocationHistory, growth: &AllocationGrowth) -> (LeakConfidence, String) {
    if history.consecutive_growths >= 3 {
        (
            LeakConfidence::High,
            format!(
                "Consistent growth across {} consecutive snapshots",
                history.consecutive_growths
            ),
        )
    } else if history.consecutive_growths >= 2 {
        (
            LeakConfidence::Medium,
            "Growth in 2 consecutive diffs".to_owned(),
        )
    } else if growth.size_diff > LARGE_GROWTH_THRESHOLD {
        let mb_whole = growth.size_diff / (1024 * 1024);
        let mb_frac = (growth.size_diff % (1024 * 1024)) * 10 / (1024 * 1024);
        (
            LeakConfidence::Medium,
            format!("Large single-diff growth ({mb_whole}.{mb_frac} MB)"),
        )
    } else {
        (LeakConfidence::Low, "May be normal cache warmup".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::super::diff::{AllocationGrowth, TraceFrame};
    use super::*;

    fn make_growth(file: &str, line: i32, size_diff: i64) -> AllocationGrowth {
        AllocationGrowth {
            file: file.to_owned(),
            line,
            size_diff,
            count_diff: 100,
            size: 1_000_000,
            count: 500,
            traceback: vec![TraceFrame {
                file: file.to_owned(),
                line,
            }],
        }
    }

    #[test]
    fn single_growth_is_low() -> Result<(), String> {
        let mut tracker = LeakTracker::new();
        let growths = vec![make_growth("src/app.py", 10, 1000)];
        let leaks = tracker.process_growths(&growths);
        assert_eq!(leaks.len(), 1);
        let first = leaks.first().ok_or("expected at least one leak")?;
        assert_eq!(first.confidence, LeakConfidence::Low);
        Ok(())
    }

    #[test]
    fn two_consecutive_growths_is_medium() -> Result<(), String> {
        let mut tracker = LeakTracker::new();
        let growths = vec![make_growth("src/app.py", 10, 1000)];
        let _ = tracker.process_growths(&growths);
        let leaks = tracker.process_growths(&growths);
        let first = leaks.first().ok_or("expected at least one leak")?;
        assert_eq!(first.confidence, LeakConfidence::Medium);
        Ok(())
    }

    #[test]
    fn three_consecutive_growths_is_high() -> Result<(), String> {
        let mut tracker = LeakTracker::new();
        let growths = vec![make_growth("src/app.py", 10, 1000)];
        let _ = tracker.process_growths(&growths);
        let _ = tracker.process_growths(&growths);
        let leaks = tracker.process_growths(&growths);
        let first = leaks.first().ok_or("expected at least one leak")?;
        assert_eq!(first.confidence, LeakConfidence::High);
        Ok(())
    }

    #[test]
    fn large_growth_is_medium() -> Result<(), String> {
        let mut tracker = LeakTracker::new();
        let growths = vec![make_growth("src/app.py", 10, 20 * 1024 * 1024)]; // 20 MB
        let leaks = tracker.process_growths(&growths);
        let first = leaks.first().ok_or("expected at least one leak")?;
        assert_eq!(first.confidence, LeakConfidence::Medium);
        Ok(())
    }

    #[test]
    fn non_growth_resets_consecutive_count() -> Result<(), String> {
        let mut tracker = LeakTracker::new();
        let growths = vec![make_growth("src/app.py", 10, 1000)];
        let _ = tracker.process_growths(&growths);
        let _ = tracker.process_growths(&growths);

        // No growth this time.
        let _ = tracker.process_growths(&[]);

        // Next growth should be Low (reset).
        let leaks = tracker.process_growths(&growths);
        let first = leaks.first().ok_or("expected at least one leak")?;
        assert_eq!(first.confidence, LeakConfidence::Low);
        Ok(())
    }

    #[test]
    fn definite_ordering() {
        assert!(LeakConfidence::Definite > LeakConfidence::High);
        assert!(LeakConfidence::High > LeakConfidence::Medium);
        assert!(LeakConfidence::Medium > LeakConfidence::Low);
    }
}
