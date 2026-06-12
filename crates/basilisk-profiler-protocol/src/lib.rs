//! Implements [PROFILE-HELPER-PROTOCOL]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-PROTOCOL
//!
//! Wire protocol shared by the Basilisk LSP and the elevated
//! `basilisk-profiler-helper` binary.
//!
//! On macOS, reading another process's memory requires elevated privileges, so
//! the LSP spawns a small helper that runs as root and streams stack samples
//! back over a Unix domain socket. Both sides MUST agree on the message shapes
//! and framing — keeping them in this single crate guarantees they never drift.
//!
//! # Framing
//!
//! Messages are newline-delimited JSON. Each side writes one compact JSON value
//! per line via [`write_message`] and reads one via [`read_message`].
//!
//! # Conversation
//!
//! ```text
//! LSP    -> {"cmd":"attach","pid":12345,"rate":100,"native":false}
//! helper -> {"type":"attached","pid":12345,"python":"3.12.0"}
//! helper -> {"type":"samples","traces":[...]}        (repeating)
//! LSP    -> {"cmd":"stop"}
//! helper -> {"type":"stopped"}
//! ```

use std::io::{Error, ErrorKind, Result};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// A command sent from the LSP to the helper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Command {
    /// Attach to a Python process and begin sampling.
    Attach {
        /// Target process ID.
        pid: u32,
        /// Samples per second (defaults to 100 when absent).
        rate: Option<u64>,
        /// Include native C frames (defaults to false when absent).
        native: Option<bool>,
    },
    /// Stop sampling and exit.
    Stop,
}

/// A message streamed from the helper back to the LSP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    /// Confirms a successful attach, reporting the detected Python version.
    Attached {
        /// Target PID.
        pid: u32,
        /// Detected Python version (e.g. `"3.12.0"`).
        python: String,
    },
    /// The attach failed; carries the real cause so the LSP can diagnose it
    /// (issue #81 — previously the helper exited without reporting and the
    /// LSP saw only an undiagnosable EOF).
    AttachFailed {
        /// Target PID.
        pid: u32,
        /// Human-readable failure cause (e.g. the py-spy attach error).
        reason: String,
    },
    /// A batch of stack-trace samples from a single sampling tick.
    Samples {
        /// The sampled traces.
        traces: Vec<TraceData>,
    },
    /// Sampling has stopped; the helper is about to exit.
    Stopped,
}

/// A single thread's stack trace, in wire form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceData {
    /// OS thread ID.
    pub thread_id: u64,
    /// Thread name if available.
    pub thread_name: Option<String>,
    /// Whether the thread is actively running (not idle).
    pub active: bool,
    /// Whether the thread holds the GIL.
    pub owns_gil: bool,
    /// Stack frames, innermost (leaf) first.
    pub frames: Vec<FrameData>,
}

/// A single stack frame, in wire form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameData {
    /// Function or method name.
    pub name: String,
    /// Source file path.
    pub filename: String,
    /// Line number in the source file.
    pub line: i32,
}

/// Write a value as one line of compact JSON followed by `\n`, then flush.
///
/// # Errors
///
/// Returns an error if serialization fails or the underlying writer errors.
pub async fn write_message<W, T>(writer: &mut W, value: &T) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
    T: Serialize + ?Sized,
{
    let json =
        serde_json::to_string(value).map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

/// Read one newline-delimited JSON value.
///
/// Returns `Ok(None)` on a clean end-of-stream (the peer closed the socket).
///
/// # Errors
///
/// Returns an error if the line cannot be read or fails to deserialize into `T`.
pub async fn read_message<R, T>(reader: &mut R) -> Result<Option<T>>
where
    R: AsyncBufReadExt + Unpin,
    T: DeserializeOwned,
{
    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line).await?;
    if bytes_read == 0 {
        return Ok(None);
    }
    let value =
        serde_json::from_str(line.trim()).map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn round_trips_attach_command() -> Result<()> {
        let cmd = Command::Attach {
            pid: 4242,
            rate: Some(200),
            native: Some(true),
        };
        let mut buf: Vec<u8> = Vec::new();
        write_message(&mut buf, &cmd).await?;
        assert!(buf.ends_with(b"\n"), "framed message must end with newline");

        let mut reader = BufReader::new(buf.as_slice());
        let decoded: Option<Command> = read_message(&mut reader).await?;
        assert_eq!(decoded, Some(cmd));
        Ok(())
    }

    #[tokio::test]
    async fn round_trips_samples_message() -> Result<()> {
        let msg = Message::Samples {
            traces: vec![TraceData {
                thread_id: 7,
                thread_name: Some("MainThread".to_owned()),
                active: true,
                owns_gil: true,
                frames: vec![FrameData {
                    name: "hot".to_owned(),
                    filename: "a.py".to_owned(),
                    line: 10,
                }],
            }],
        };
        let mut buf: Vec<u8> = Vec::new();
        write_message(&mut buf, &msg).await?;
        let mut reader = BufReader::new(buf.as_slice());
        let decoded: Option<Message> = read_message(&mut reader).await?;
        assert_eq!(decoded, Some(msg));
        Ok(())
    }

    #[tokio::test]
    async fn read_message_returns_none_on_eof() -> Result<()> {
        let empty: &[u8] = b"";
        let mut reader = BufReader::new(empty);
        let decoded: Option<Message> = read_message(&mut reader).await?;
        assert_eq!(decoded, None);
        Ok(())
    }

    #[tokio::test]
    async fn read_message_errors_on_garbage() {
        let garbage: &[u8] = b"not json\n";
        let mut reader = BufReader::new(garbage);
        let decoded: Result<Option<Message>> = read_message(&mut reader).await;
        assert!(decoded.is_err(), "malformed JSON must surface an error");
    }
}
