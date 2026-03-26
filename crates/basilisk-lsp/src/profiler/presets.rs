//! Profiling presets for common use cases.
//!
//! Each preset bundles a sample rate and optional duration limit into a
//! [`SamplerConfig`] so users can select a pre-configured profiling mode
//! instead of tuning individual parameters.

use std::time::Duration;

use super::sampler::SamplerConfig;

/// Pre-configured profiling modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilingPreset {
    /// Short burst: 10 seconds at 100 Hz. Good for quick hotspot checks.
    Quick,
    /// Thorough analysis: 60 seconds at 200 Hz. Higher fidelity data.
    Detailed,
    /// No time limit at 50 Hz. For long-running servers and batch jobs.
    LongRunning,
    /// User-specified parameters (not a preset).
    Custom,
}

impl ProfilingPreset {
    /// Parse a preset name from a JSON string value.
    ///
    /// Returns `None` for unrecognized names or `"custom"`.
    #[must_use]
    pub fn parse_name(name: &str) -> Option<Self> {
        match name {
            "quick" => Some(Self::Quick),
            "detailed" => Some(Self::Detailed),
            "longRunning" | "long_running" => Some(Self::LongRunning),
            _ => None,
        }
    }
}

/// Build a [`SamplerConfig`] for the given preset and target PID.
#[must_use]
pub fn preset_config(preset: ProfilingPreset, pid: u32) -> SamplerConfig {
    match preset {
        ProfilingPreset::Quick => SamplerConfig {
            pid,
            sample_rate: 100,
            include_native: false,
            include_idle: false,
            duration: Some(Duration::from_secs(10)),
        },
        ProfilingPreset::Detailed => SamplerConfig {
            pid,
            sample_rate: 200,
            include_native: false,
            include_idle: false,
            duration: Some(Duration::from_secs(60)),
        },
        ProfilingPreset::LongRunning => SamplerConfig {
            pid,
            sample_rate: 50,
            include_native: false,
            include_idle: false,
            duration: None,
        },
        ProfilingPreset::Custom => SamplerConfig {
            pid,
            ..SamplerConfig::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_preset_has_10s_duration_100hz() {
        let config = preset_config(ProfilingPreset::Quick, 1234);
        assert_eq!(config.pid, 1234);
        assert_eq!(config.sample_rate, 100);
        assert_eq!(config.duration, Some(Duration::from_secs(10)));
        assert!(!config.include_native);
    }

    #[test]
    fn detailed_preset_has_60s_duration_200hz() {
        let config = preset_config(ProfilingPreset::Detailed, 5678);
        assert_eq!(config.pid, 5678);
        assert_eq!(config.sample_rate, 200);
        assert_eq!(config.duration, Some(Duration::from_secs(60)));
    }

    #[test]
    fn long_running_preset_has_no_duration_50hz() {
        let config = preset_config(ProfilingPreset::LongRunning, 9999);
        assert_eq!(config.sample_rate, 50);
        assert!(config.duration.is_none());
    }

    #[test]
    fn custom_preset_uses_defaults() {
        let config = preset_config(ProfilingPreset::Custom, 42);
        assert_eq!(config.pid, 42);
        assert_eq!(config.sample_rate, SamplerConfig::default().sample_rate);
    }

    #[test]
    fn parse_preset_names() {
        assert_eq!(
            ProfilingPreset::parse_name("quick"),
            Some(ProfilingPreset::Quick)
        );
        assert_eq!(
            ProfilingPreset::parse_name("detailed"),
            Some(ProfilingPreset::Detailed)
        );
        assert_eq!(
            ProfilingPreset::parse_name("longRunning"),
            Some(ProfilingPreset::LongRunning)
        );
        assert_eq!(
            ProfilingPreset::parse_name("long_running"),
            Some(ProfilingPreset::LongRunning)
        );
        assert_eq!(ProfilingPreset::parse_name("custom"), None);
        assert_eq!(ProfilingPreset::parse_name("unknown"), None);
    }
}
