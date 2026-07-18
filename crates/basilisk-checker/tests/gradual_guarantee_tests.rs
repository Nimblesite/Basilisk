//! Tests for [TYPEINF-TARGET-GRADUAL]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-GRADUAL and the
//! Stage 0 checklist in
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//!
//! The gradual guarantee is a differential invariant: **removing an
//! annotation must never introduce a new static error**. The harness strips
//! annotations from a corpus (byte-preserving — every removed span is
//! overwritten with spaces, so all other diagnostic spans stay comparable),
//! re-checks with the default config, and asserts the stripped file's error
//! set is a subset of the annotated file's.
#![expect(
    clippy::expect_used,
    reason = "test-only parsing of fixed, known-valid corpus fixtures"
)]

use std::collections::BTreeSet;

use ruff_python_ast::visitor::{walk_stmt, Visitor};
use ruff_python_ast::{Parameter, Stmt};
use ruff_text_size::{Ranged, TextRange};

/// Collect the byte ranges of removable annotations in one module.
#[derive(Default)]
struct AnnotationRanges<'src> {
    ranges: Vec<TextRange>,
    source: &'src str,
}

impl AnnotationRanges<'_> {
    /// Record the `: T` span of one parameter (name end → annotation end,
    /// which covers the colon).
    fn record_parameter(&mut self, parameter: &Parameter) {
        if let Some(annotation) = &parameter.annotation {
            self.ranges.push(TextRange::new(
                parameter.name.range().end(),
                annotation.range().end(),
            ));
        }
    }

    /// Whether a class body is a **declaration interface** — a `Protocol`,
    /// `TypedDict`, or `NamedTuple`. Annotations there define the structural
    /// interface itself (member types, protocol variance inference per PEP
    /// 544), so removing them yields a *different* interface, not the same
    /// program with fewer hints. The gradual guarantee
    /// ([TYPEINF-TARGET-GRADUAL]) is about inferable positions; these bodies
    /// are excluded from stripping.
    fn is_declaration_interface(&self, def: &ruff_python_ast::StmtClassDef) -> bool {
        let base_is_interface = def.bases().iter().any(|base| {
            let range = base.range();
            self.source
                .get(usize::from(range.start())..usize::from(range.end()))
                .is_some_and(|text| {
                    text.contains("Protocol")
                        || text.contains("TypedDict")
                        || text.contains("NamedTuple")
                })
        });
        // A dataclass body is declaration syntax at RUNTIME (PEP 557): only
        // annotated assignments become fields, so `x: int = 1` and `x = 1`
        // are different programs, not the same program with fewer hints.
        let is_dataclass = def.decorator_list.iter().any(|decorator| {
            let range = decorator.range();
            self.source
                .get(usize::from(range.start())..usize::from(range.end()))
                .is_some_and(|text| text.contains("dataclass"))
        });
        base_is_interface || is_dataclass
    }
}

impl Visitor<'_> for AnnotationRanges<'_> {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::ClassDef(def) if self.is_declaration_interface(def) => {
                // Do not recurse: every annotation inside is load-bearing.
                return;
            }
            // `@overload` signatures are declaration syntax too — the
            // annotations ARE the overload (PEP 484 §overloading). Stripping
            // them produces a different (and typically inconsistent) overload
            // set, not the same program with fewer hints.
            Stmt::FunctionDef(def)
                if def.decorator_list.iter().any(|decorator| {
                    let range = decorator.range();
                    self.source
                        .get(usize::from(range.start())..usize::from(range.end()))
                        .is_some_and(|text| text.contains("overload"))
                }) =>
            {
                return;
            }
            Stmt::FunctionDef(def) => {
                for parameter in &def.parameters {
                    self.record_parameter(parameter.as_parameter());
                }
                if let Some(returns) = &def.returns {
                    // The `->` arrow before the annotation is blanked by the
                    // splicer's back-scan; only the annotation span is recorded.
                    self.ranges.push(returns.range());
                }
            }
            // `x: T = value` → `x = value`. Bare `x: T` (no value) IS the
            // declaration — nothing would remain after removal, so keep it.
            // `Final`/`ClassVar` are semantic QUALIFIERS (PEP 591/526): they
            // change what the program may legally do (reassignment, instance
            // vs class storage), not just the inferable type — removing them
            // is a semantic edit, so they stay.
            Stmt::AnnAssign(ann) if ann.value.is_some() => {
                let annotation_range = ann.annotation.range();
                let is_qualifier = self
                    .source
                    .get(usize::from(annotation_range.start())..usize::from(annotation_range.end()))
                    .is_some_and(|text| text.contains("Final") || text.contains("ClassVar"));
                if !is_qualifier {
                    self.ranges.push(TextRange::new(
                        ann.target.range().end(),
                        annotation_range.end(),
                    ));
                }
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

/// Blank `range` (with a back-scan over any `->` arrow and whitespace before
/// it) in `bytes`, preserving newlines so line structure never shifts.
fn blank_range(bytes: &mut [u8], range: TextRange) {
    let start = usize::from(range.start());
    let end = usize::from(range.end()).min(bytes.len());
    // Back-scan: blank a preceding `->` (and the whitespace around it) so a
    // return annotation removal leaves `def f(a)      :` rather than `-> :`.
    let mut arrow = start;
    while arrow > 0 && bytes.get(arrow - 1).is_some_and(|b| *b == b' ') {
        arrow -= 1;
    }
    let arrow_start = arrow
        .checked_sub(2)
        .filter(|candidate| bytes.get(*candidate..arrow).is_some_and(|s| s == b"->"));
    for byte in bytes
        .get_mut(arrow_start.unwrap_or(start)..end)
        .unwrap_or_default()
    {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

/// Strip every removable annotation from `source`, byte-preserving.
///
/// Annotations whose span contains a newline are kept (blanking them would
/// merge physical lines and could leave dangling continuation syntax); the
/// guarantee is still exercised by every single-line annotation around them.
fn strip_annotations(source: &str) -> String {
    let parsed = ruff_python_parser::parse_module(source).expect("corpus fixture must parse");
    let mut collector = AnnotationRanges {
        ranges: Vec::new(),
        source,
    };
    for stmt in &parsed.syntax().body {
        collector.visit_stmt(stmt);
    }
    let mut bytes = source.as_bytes().to_vec();
    for range in collector.ranges {
        let start = usize::from(range.start());
        let end = usize::from(range.end()).min(bytes.len());
        let spans_newline = bytes
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .any(|b| *b == b'\n' || *b == b'\r' || *b == b'\\');
        if !spans_newline {
            blank_range(&mut bytes, range);
        }
    }
    String::from_utf8(bytes).expect("blanking preserves UTF-8")
}

/// Rules that judge the coherence of the annotation web ITSELF (`TypeVar`
/// scoping, variance inference over declared signatures) rather than values
/// against types. Removing a subset of annotations can leave the remaining
/// web ill-formed — e.g. a body annotation referencing a `TypeVarTuple` whose
/// only binding site was a stripped parameter — so these are outside the
/// gradual guarantee's scope (which is about inferable value positions) and
/// excluded from the differential.
const ANNOTATION_WEB_RULES: &[&str] = &["generics_scoping", "generics_variance_inference"];

/// Check `source` with the default config; return `(code, span-start)` pairs.
///
/// Byte-preserving stripping keeps all spans stable, so this pair identifies
/// "the same error at the same place" across the annotated/stripped variants.
fn error_set(source: &str, path: &str) -> BTreeSet<(String, u32)> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .expect("corpus variant must parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("corpus variant must resolve");
    basilisk_checker::check(&resolved)
        .into_iter()
        .filter(|diagnostic| matches!(diagnostic.severity, basilisk_checker::Severity::Error))
        .filter(|diagnostic| !ANNOTATION_WEB_RULES.contains(&diagnostic.code.code))
        .map(|diagnostic| (diagnostic.code.code.to_owned(), diagnostic.span.start))
        .collect()
}

/// The differential assertion: stripping annotations must not add errors.
fn assert_gradual_guarantee(source: &str, name: &str) {
    let annotated = error_set(source, name);
    let stripped_source = strip_annotations(source);
    let stripped = error_set(&stripped_source, name);
    let new_errors: Vec<_> = stripped.difference(&annotated).collect();
    assert!(
        new_errors.is_empty(),
        "[TYPEINF-TARGET-GRADUAL] violated in {name}: stripping annotations \
         introduced new errors {new_errors:?}\n--- stripped source ---\n{stripped_source}"
    );
}

/// Curated corpus: annotated programs exercising containers, unions,
/// optionals, callables, comprehensions, and class bodies.
const CURATED_CORPUS: &[(&str, &str)] = &[
    (
        "containers",
        r#"
from typing import Optional

def totals(prices: list[float], names: list[str]) -> dict[str, float]:
    table: dict[str, float] = {}
    for name, price in zip(names, prices):
        table[name] = price
    return table

counts: list[int] = [1, 2, 3]
maybe: Optional[int] = None
pairs: dict[str, int] = {"a": 1}
"#,
    ),
    (
        "callables_and_lambdas",
        r"
from typing import Callable

def apply(f: Callable[[int], int], x: int) -> int:
    return f(x)

double: Callable[[int], int] = lambda n: n * 2
result: int = apply(double, 21)
",
    ),
    (
        "unions_and_narrowing",
        r"
def describe(value: int | str | None) -> str:
    if value is None:
        return 'nothing'
    if isinstance(value, int):
        return str(value + 1)
    return value
",
    ),
    (
        "comprehensions",
        r"
def squares(limit: int) -> list[int]:
    values: list[int] = [n * n for n in range(limit)]
    return values

index: dict[str, int] = {str(n): n for n in range(3)}
",
    ),
    (
        "class_bodies",
        r"
class Point:
    x: float
    y: float

    def __init__(self, x: float, y: float) -> None:
        self.x = x
        self.y = y

    def magnitude_squared(self) -> float:
        total: float = self.x * self.x + self.y * self.y
        return total
",
    ),
    (
        "intentional_errors_stay_bounded",
        r#"
def bad() -> int:
    value: int = "not an int"
    return value
"#,
    ),
];

/// [TYPEINF-TARGET-GRADUAL]: the differential harness over the curated corpus.
#[test]
fn stripping_annotations_never_adds_errors_curated() {
    for (name, source) in CURATED_CORPUS {
        assert_gradual_guarantee(source, name);
    }
}

/// The splicer itself must produce parseable Python for every corpus entry —
/// a splice bug would silently shrink the differential to nothing.
#[test]
fn stripped_corpus_still_parses_and_actually_stripped() {
    for (name, source) in CURATED_CORPUS {
        let stripped = strip_annotations(source);
        assert!(
            ruff_python_parser::parse_module(&stripped).is_ok(),
            "{name}: stripped variant must parse\n{stripped}"
        );
        assert_eq!(
            stripped.len(),
            source.len(),
            "{name}: stripping must be byte-preserving"
        );
        if source.contains("->") {
            assert!(
                !stripped.contains("->"),
                "{name}: single-line return annotations must be stripped\n{stripped}"
            );
        }
    }
}

/// Sweep the mirrored conformance fixtures when present (they are synced by
/// `scripts/test-rust.sh` and git-ignored). Every fixture that parses both
/// annotated and stripped must satisfy the guarantee — these files are dense
/// with intentional errors, making them a strong differential corpus.
#[test]
fn stripping_annotations_never_adds_errors_conformance_fixtures() {
    let fixtures_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/tests");
    let Ok(entries) = std::fs::read_dir(&fixtures_dir) else {
        // Fixtures not synced in this environment — the curated corpus above
        // still enforces the invariant.
        return;
    };
    let mut checked = 0_u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "py") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Skip fixtures the annotated pipeline cannot process (syntax-error
        // fixtures); the guarantee is about programs that check today.
        if basilisk_parser::parse_source(source.clone(), path.display().to_string()).is_err() {
            continue;
        }
        let stripped_source = strip_annotations(&source);
        if ruff_python_parser::parse_module(&stripped_source).is_err() {
            continue;
        }
        assert_gradual_guarantee(&source, &path.display().to_string());
        checked += 1;
    }
    assert!(
        checked > 0,
        "conformance fixtures were present but none were checkable"
    );
}
