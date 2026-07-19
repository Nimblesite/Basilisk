//! External tests for [TYPEINF-NARROWING-ATTR-CALLS]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ATTR-CALLS.

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject(PathBuf);

impl Drop for TempProject {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn attribute_narrowing_call_policy_is_parsed_without_claiming_checker_semantics(
) -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let project = TempProject(std::env::temp_dir().join(format!(
        "basilisk-attr-narrowing-config-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&project.0)?;
    fs::write(
        project.0.join("pyproject.toml"),
        "[tool.basilisk]\nnarrow-attributes-across-calls = false\n",
    )?;

    let config = basilisk_config::load_basilisk_config(&project.0);
    assert_eq!(config.narrow_attributes_across_calls, Some(false));
    Ok(())
}
