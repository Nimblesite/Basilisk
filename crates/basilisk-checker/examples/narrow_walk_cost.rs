//! Implements the [NARROWPLAN-INTEGRATION] cost measurement
//! (docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION):
//! does one flow walk cost more as the surrounding MODULE grows?
//!
//! The walker's expression synthesis is seeded with the module's callable
//! interfaces. Rebuilding that seed per expression made a single function's
//! walk scale with the whole file's size — invisible to `make _bench`, which
//! times `basilisk check` and never enters this code. This example is the
//! harness that made the cost visible, and the regression check for it: the
//! reported time must stay flat as `callables` grows.
//!
//! Usage (self-measured, methodology as stated — NOT a competitor comparison):
//! ```sh
//! cargo run --release -p basilisk-checker --example narrow_walk_cost
//! ```
//!
//! The fixture is one function of `BRANCHES` guarded blocks, each containing a
//! call, an early `return`, and a list literal — so every walk drives the
//! divergence probe, the branch/complement machinery, and expression synthesis
//! many times over. The module around it holds N callables the function never
//! mentions.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Instant;

use basilisk_checker::narrow::{analyse_function_in, NarrowContext, NarrowEnv};
use basilisk_checker::types::InferredType;
use ruff_python_ast::Stmt;

/// Guarded blocks in the measured function.
const BRANCHES: usize = 60;

/// Walks per measurement, averaged.
const REPEATS: u32 = 20;

/// Module sizes to measure the walk against.
const MODULE_SIZES: [usize; 4] = [0, 100, 1_000, 5_000];

/// The measured function: `BRANCHES` narrowing blocks over two optionals.
fn fixture() -> Result<String, std::fmt::Error> {
    let mut source = String::from("def f(x: int | None, y: str | None) -> int:\n");
    for index in 0..BRANCHES {
        write!(
            source,
            "    if x is None:\n        a{index} = helper()\n        return 0\n    b{index} = [x, x]\n"
        )?;
    }
    source.push_str("    return 1\n");
    Ok(source)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = fixture()?;

    let parsed = basilisk_parser::parse_source(source.clone(), "cost.py".to_owned())
        .map_err(|error| format!("fixture must parse: {error}"))?;
    let resolved = basilisk_resolver::resolve(&parsed)
        .map_err(|error| format!("fixture must resolve: {error}"))?;
    let function = resolved
        .functions
        .first()
        .ok_or("fixture must contain one function")?;

    let reparsed = ruff_python_parser::parse_module(&source)
        .map_err(|error| format!("fixture must reparse: {error}"))?;
    let body = reparsed
        .syntax()
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::FunctionDef(def) => Some(def.body.to_vec()),
            _ => None,
        })
        .ok_or("fixture must contain one function body")?;

    let declared: HashMap<String, InferredType> = [
        (
            "x".to_owned(),
            InferredType::Optional(Box::new(InferredType::Int)),
        ),
        (
            "y".to_owned(),
            InferredType::Optional(Box::new(InferredType::Str)),
        ),
    ]
    .into_iter()
    .collect();

    println!("fixture: {BRANCHES} guarded blocks, {REPEATS} walks averaged");
    for size in MODULE_SIZES {
        let ctx = NarrowContext {
            callables: (0..size)
                .map(|index| (format!("unused{index}"), InferredType::Int))
                .collect(),
            ..Default::default()
        };
        let start = Instant::now();
        // Every walk is deterministic, so the last count IS the count — it is
        // reported so a "faster" run that stopped narrowing cannot pass unseen.
        let mut narrowed = 0;
        for _ in 0..REPEATS {
            let result = analyse_function_in(
                &body,
                NarrowEnv::new(declared.clone()),
                &function.narrowing_guards,
                &ctx,
            );
            narrowed = result.narrowed_uses.len();
        }
        println!(
            "RESULT module_callables={size} per_walk={:?} narrowed_uses={narrowed}",
            start.elapsed() / REPEATS,
        );
    }
    Ok(())
}
