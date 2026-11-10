//! PEP 484: version and platform guards are statically evaluated for a configured target.
//! Specification: https://peps.python.org/pep-0484/#version-and-platform-checking

mod common;

use common::{assert_rule_count, run, run_with_config};

const RULE: &str = "directives_version_platform";

fn target(version: Option<&str>, platform: Option<&str>) -> basilisk_config::BasiliskConfig {
    basilisk_config::BasiliskConfig {
        python_version: version.map(str::to_owned),
        python_platform: platform.map(str::to_owned),
        ..Default::default()
    }
}

#[test]
fn configured_version_makes_the_impossible_branch_dead() -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        r#"
import sys

if sys.version_info < (3, 8):
    obsolete = 1

print(obsolete)
"#,
        r#"
import sys as runtime

if runtime.version_info < (3, 8):
    obsolete = 1

print(obsolete)
"#,
    ] {
        let diagnostics = run_with_config(source, &target(Some("3.12"), None))?;
        assert_rule_count(
            &diagnostics,
            RULE,
            1,
            "PEP 484 permits a checker to treat a false version-guard branch as unreachable",
        );
    }
    Ok(())
}

#[test]
fn configured_platform_makes_the_impossible_branch_dead() -> Result<(), Box<dyn std::error::Error>>
{
    let diagnostics = run_with_config(
        r#"
import sys

if sys.platform == "win32":
    windows_only = 1

print(windows_only)
"#,
        &target(None, Some("linux")),
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 484 permits a checker to evaluate sys.platform for its configured target",
    );
    Ok(())
}

#[test]
fn absent_target_evidence_does_not_make_either_branch_dead(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
import sys

if sys.version_info < (3, 8):
    old = 1
else:
    new = 1

print(old, new)
"#)?;
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "without a configured target, PEP 484 does not justify declaring either branch dead",
    );
    Ok(())
}
