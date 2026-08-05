//! Tests for the BSK-0001 `param_infer` exemption — [NARROWPLAN-INTEGRATION]
//! Step 6, [TYPEINF-EXCEEDS-REQUIRED]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! [#317](https://github.com/Nimblesite/Basilisk/issues/317): BSK-0001 must
//! not demand an annotation the engine already infers from body constraints
//! or same-module call sites — and must keep firing where there is no
//! evidence.

use super::common::*;

/// A body constraint pins the parameter: passing it to a callee with a
/// declared parameter type demands that type, so the annotation is
/// inferable and BSK-0001 stays silent.
#[test]
fn body_demand_exempts_the_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def consume(value: int) -> bool:
    return value > 0


def wrapper(p):
    return consume(p)
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let fired: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .map(|d| d.message.clone())
        .collect();
    assert!(
        fired.is_empty(),
        "a body-demanded parameter type is inferable — BSK-0001 must stay silent, got: {fired:?}"
    );
    Ok(())
}

/// No evidence at all: the annotation demand stands.
#[test]
fn no_evidence_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def orphan(p):
    return p
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0001"),
        "a parameter with no inference evidence must keep firing, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Same-module call sites supply lower bounds that pin the parameter.
#[test]
fn call_site_evidence_exempts_the_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def double(p):
    return p


double(1)
double(2)
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let fired: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .map(|d| d.message.clone())
        .collect();
    assert!(
        fired.is_empty(),
        "call-site-typed parameters are inferable — BSK-0001 must stay silent, got: {fired:?}"
    );
    Ok(())
}

/// Methods are outside `param_infer`'s reach — they keep firing unchanged.
#[test]
fn method_parameters_keep_firing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Box:
    def put(self, item) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0001"),
        "an uninferable method parameter must keep firing, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
