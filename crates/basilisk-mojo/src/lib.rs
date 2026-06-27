//! Implements [CHKARCH-MOJO-SAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-MOJO-SAFETY
//! Mojo-inspired ownership and immutability analysis for Basilisk.
//!
//! Houses ownership tracking, immutability enforcement, and coercion
//! detection (BSK-E003x, BSK-E004x, BSK-E006x) in Phase 4.

/// Mutating methods that modify a collection in place.
const MUTATING_METHODS: &[&str] = &[
    "append", "extend", "insert", "remove", "pop", "clear", "sort", "reverse", "update", "add",
    "discard",
];

/// Check a Python source string for Mojo-style ownership violations.
///
/// Detects mutation of `Borrowed` parameters via mutating method calls
/// (BSK-E003x / BSK-E004x / BSK-E006x).
#[must_use]
pub fn check_ownership(source: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let borrowed_params = collect_borrowed_params(source);

    if borrowed_params.is_empty() {
        return violations;
    }

    for line in source.lines() {
        let trimmed = line.trim();
        for param in &borrowed_params {
            for method in MUTATING_METHODS {
                let pattern = format!("{param}.{method}(");
                if trimmed.contains(&pattern) {
                    violations.push(format!(
                        "directives_cast: mutation of Borrowed parameter `{param}` \
                         via `.{method}()` is not allowed"
                    ));
                }
            }
        }
    }

    violations
}

/// Extract parameter names annotated with `Borrowed[...]` from source text.
fn collect_borrowed_params(source: &str) -> Vec<String> {
    let mut params = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("def ") {
            continue;
        }
        let Some(open) = trimmed.find('(') else {
            continue;
        };
        let Some(close) = trimmed.rfind(')') else {
            continue;
        };
        if open >= close {
            continue;
        }

        let param_str = &trimmed[open + 1..close];
        for part in param_str.split(',') {
            let part = part.trim();
            if let Some(colon_pos) = part.find(':') {
                let name = part[..colon_pos].trim();
                let annotation = part[colon_pos + 1..].trim();
                if annotation.starts_with("Borrowed[") || annotation == "Borrowed" {
                    params.push(name.to_string());
                }
            }
        }
    }

    params
}
