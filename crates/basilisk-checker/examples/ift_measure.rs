//! Implements the [NARROWPLAN-CHECKLIST] Stage 2 measurement item: narrowing
//! richness against the utahplt/ifT-benchmark
//! (<https://github.com/utahplt/ift-benchmark>), whose Python suite
//! (`Pyright/main.py`) is built on `type(x) is C` guards over `@final`
//! classes ([TYPEINF-NARROWING-TYPEOF]).
//!
//! Usage (clone fresh, then measure):
//! ```sh
//! git clone --depth 1 https://github.com/utahplt/ift-benchmark /tmp/ift
//! cargo run --release -p basilisk-checker --example ift_measure -- /tmp/ift/Pyright/main.py
//! ```
//!
//! Metric (self-measured, stated methodology): for every function in the
//! suite, run parse → resolve → flow analysis and report whether at least
//! one flow-narrowed use is produced. This measures narrowing *richness*
//! (which guard forms produce narrowing at all) — it is NOT the benchmark's
//! own accept/reject score, which needs the full diagnostics pipeline and
//! lands with the Integration stage.

use std::collections::HashMap;

use basilisk_checker::narrow::{analyse_function_in, NarrowContext, NarrowEnv};
use basilisk_checker::types::InferredType;
use ruff_python_ast::Stmt;

/// Declared parameter types for one resolved function.
fn declared_types(
    source: &str,
    function: &basilisk_resolver::FunctionInfo,
) -> HashMap<String, InferredType> {
    function
        .parameters
        .iter()
        .filter_map(|param| {
            let span = param.annotation_span?;
            let start = usize::try_from(span.start).ok()?;
            let end = usize::try_from(span.end).ok()?;
            let text = source.get(start..end)?;
            Some((param.name.clone(), InferredType::from_annotation(text)))
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: ift_measure <path-to-ift-benchmark Pyright/main.py>")?;
    let source = std::fs::read_to_string(&path)?;

    let parsed = basilisk_parser::parse_source(source.clone(), path)
        .map_err(|error| format!("benchmark file must parse: {error}"))?;
    let resolved = basilisk_resolver::resolve(&parsed)
        .map_err(|error| format!("benchmark file must resolve: {error}"))?;

    // The suite's `@final` classes make negative `type(x) is not C` sound.
    let mut ctx = NarrowContext::default();
    for class in &resolved.classes {
        let _ = ctx.final_classes.insert(class.name.to_ascii_lowercase());
    }

    let reparsed = ruff_python_parser::parse_module(&source)
        .map_err(|error| format!("benchmark file must reparse: {error}"))?;
    let bodies: HashMap<String, Vec<Stmt>> = reparsed
        .syntax()
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::FunctionDef(def) => Some((def.name.to_string(), def.body.to_vec())),
            _ => None,
        })
        .collect();

    let mut narrowed = Vec::new();
    let mut silent = Vec::new();
    for function in &resolved.functions {
        if function.class_name.is_some() {
            continue;
        }
        let Some(body) = bodies.get(&function.name) else {
            continue;
        };
        let declared = declared_types(&source, function);
        let result = analyse_function_in(
            body,
            NarrowEnv::new(declared),
            &function.narrowing_guards,
            &ctx,
        );
        if result.narrowed_uses.is_empty() && result.unreachable_ranges.is_empty() {
            silent.push(function.name.clone());
        } else {
            narrowed.push(function.name.clone());
        }
    }

    narrowed.sort();
    silent.sort();
    println!("functions with narrowing signal: {}", narrowed.len());
    println!("functions without narrowing signal: {}", silent.len());
    println!("silent: {silent:?}");
    println!(
        "RESULT narrowing_signal={} total={} pct={}",
        narrowed.len(),
        narrowed.len() + silent.len(),
        narrowed.len() * 100 / (narrowed.len() + silent.len()).max(1)
    );
    Ok(())
}
