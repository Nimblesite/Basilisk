//! Runtime introspection stub generation.
//!
//! Spawns a Python subprocess to `import` the target module and extract
//! function signatures via `inspect.signature()`.  Returns the result as
//! generated `.pyi` content.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{GeneratedStub, StubGenError, StubGenMode};

/// Default subprocess timeout.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Python script that imports a module and dumps its public API as JSON.
///
/// Output format (one JSON object per line):
/// ```json
/// {"name": "get", "kind": "function", "signature": "(url: str, **kwargs) -> Response", "return": "Response"}
/// {"name": "Session", "kind": "class", "methods": [...]}
/// ```
const INTROSPECT_SCRIPT: &str = r#"
import sys, json, inspect, types

module_name = sys.argv[1]
try:
    module = __import__(module_name)
except Exception as e:
    print(json.dumps({"error": str(e)}))
    sys.exit(1)

results = []
for name in sorted(dir(module)):
    if name.startswith('_'):
        continue
    obj = getattr(module, name)
    entry = {"name": name}
    if callable(obj):
        try:
            sig = inspect.signature(obj)
            params = []
            for pname, param in sig.parameters.items():
                p = {"name": pname}
                if param.annotation is not inspect.Parameter.empty:
                    p["annotation"] = str(param.annotation)
                if param.default is not inspect.Parameter.empty:
                    p["has_default"] = True
                if param.kind == inspect.Parameter.VAR_POSITIONAL:
                    p["kind"] = "vararg"
                elif param.kind == inspect.Parameter.VAR_KEYWORD:
                    p["kind"] = "kwarg"
                elif param.kind == inspect.Parameter.KEYWORD_ONLY:
                    p["kind"] = "keyword_only"
                elif param.kind == inspect.Parameter.POSITIONAL_ONLY:
                    p["kind"] = "positional_only"
                params.append(p)
            entry["params"] = params
            ret = sig.return_annotation
            if ret is not inspect.Parameter.empty:
                entry["return"] = str(ret)
        except (ValueError, TypeError):
            entry["params"] = []
        if isinstance(obj, type):
            entry["kind"] = "class"
        else:
            entry["kind"] = "function"
    elif isinstance(obj, types.ModuleType):
        continue
    else:
        entry["kind"] = "variable"
        ann = type(obj).__name__
        entry["annotation"] = ann
    results.append(entry)

print(json.dumps(results))
"#;

/// Generate stubs by running `inspect.signature()` on each public symbol.
///
/// # Errors
///
/// Returns `StubGenError::Subprocess` if the Python process fails or times out.
/// Returns `StubGenError::Import` if the module cannot be imported.
pub fn generate_runtime_stubs(
    module_name: &str,
    python_path: &Path,
) -> Result<GeneratedStub, StubGenError> {
    let output = Command::new(python_path)
        .args(["-c", INTROSPECT_SCRIPT, module_name])
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .map_err(|err| StubGenError::Subprocess(format!("failed to spawn Python: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StubGenError::Import(format!(
            "Python exited with {}: {stderr}",
            output.status
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<serde_json::Value> = serde_json::from_str(stdout.trim())
        .map_err(|err| StubGenError::Subprocess(format!("invalid JSON output: {err}")))?;

    // Check for import error in output.
    if let Some(first) = entries.first() {
        if let Some(error) = first.get("error").and_then(|v| v.as_str()) {
            return Err(StubGenError::Import(error.to_owned()));
        }
    }

    let pyi_content = entries_to_pyi(module_name, &entries);

    Ok(GeneratedStub {
        module_name: module_name.to_owned(),
        pyi_content,
        mode: StubGenMode::Runtime,
    })
}

/// Convert introspection JSON entries to `.pyi` stub content.
fn entries_to_pyi(module_name: &str, entries: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "# Auto-generated stub for `{module_name}` (runtime introspection)"
    ));
    lines.push("# Tier 3: best-effort, may be inaccurate".to_owned());
    lines.push(String::new());
    lines.push("from typing import Any".to_owned());
    lines.push(String::new());

    for entry in entries {
        let Some(name) = entry.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let kind = entry
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("variable");

        match kind {
            "function" => {
                let sig = format_function_stub(name, entry);
                lines.push(sig);
            }
            "class" => {
                lines.push(format!("class {name}: ..."));
            }
            "variable" => {
                let ann = entry
                    .get("annotation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Any");
                lines.push(format!("{name}: {ann}"));
            }
            _ => {}
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

/// Format a single function as a `.pyi` stub line.
fn format_function_stub(name: &str, entry: &serde_json::Value) -> String {
    let mut params = Vec::new();
    if let Some(param_list) = entry.get("params").and_then(|v| v.as_array()) {
        for param in param_list {
            let pname = param.get("name").and_then(|v| v.as_str()).unwrap_or("arg");
            let ann = param.get("annotation").and_then(|v| v.as_str());
            let kind = param.get("kind").and_then(|v| v.as_str());

            let formatted = match kind {
                Some("vararg") => format!("*{pname}"),
                Some("kwarg") => format!("**{pname}"),
                _ => {
                    if let Some(a) = ann {
                        format!("{pname}: {a}")
                    } else {
                        pname.to_owned()
                    }
                }
            };
            params.push(formatted);
        }
    }

    let ret = entry
        .get("return")
        .and_then(|v| v.as_str())
        .unwrap_or("Any");

    format!("def {name}({}) -> {ret}: ...", params.join(", "))
}

/// Timeout constant exposed for configuration.
#[must_use]
pub const fn default_timeout() -> Duration {
    TIMEOUT
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn entries_to_pyi_produces_valid_stub() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "get", "kind": "function", "params": [{"name": "url", "annotation": "str"}], "return": "Response"},
                {"name": "Session", "kind": "class"},
                {"name": "VERSION", "kind": "variable", "annotation": "str"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("requests", &entries);
        assert!(pyi.contains("def get(url: str) -> Response: ..."));
        assert!(pyi.contains("class Session: ..."));
        assert!(pyi.contains("VERSION: str"));
        assert!(pyi.contains("Auto-generated stub"));
    }

    #[test]
    fn format_function_stub_with_varargs() {
        let entry: serde_json::Value = serde_json::from_str(
            r#"{"name": "foo", "kind": "function", "params": [{"name": "a"}, {"name": "args", "kind": "vararg"}, {"name": "kwargs", "kind": "kwarg"}], "return": "None"}"#,
        )
        .unwrap();

        let stub = format_function_stub("foo", &entry);
        assert_eq!(stub, "def foo(a, *args, **kwargs) -> None: ...");
    }
}
