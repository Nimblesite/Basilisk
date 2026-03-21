//! Error types for uv integration.

/// Errors that can occur during uv project detection, parsing, or registry
/// construction.
#[derive(Debug, thiserror::Error)]
pub enum UvError {
    /// Failed to read a file from disk.
    #[error("failed to read file: {path}")]
    Io {
        /// The path that could not be read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse a TOML file.
    #[error("failed to parse TOML: {path}")]
    TomlParse {
        /// The path of the malformed TOML file.
        path: String,
        /// The underlying TOML deserialization error.
        #[source]
        source: toml::de::Error,
    },

    /// Failed to parse the uv lock file structure.
    #[error("failed to parse lock file: {0}")]
    LockFileParse(String),
}
