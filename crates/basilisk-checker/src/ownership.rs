//! Implements [CHKARCH-SAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-SAFETY
//!
//! Basilisk's own ownership analysis: scaffolding for the planned
//! mutation-of-borrowed diagnostic ([CHKARCH-SAFETY-OWNERSHIP]). The concepts
//! are inspired by Mojo's ownership model; the rules are Basilisk's, spelled
//! in standard Python `Annotated` conventions.
//!
//! This is **not** wired into the checker pipeline and registers no rule, so it
//! never contributes a diagnostic — shipping PEP rules must not reuse these
//! anchors or descriptions. There is no separate crate for it: the analysis
//! lives here, alongside the rest of the checking logic.
//!
//! The scan is textual rather than AST-driven, so it sees `Borrowed` spellings
//! inside comments and strings and misses defs spanning several lines. That is
//! adequate for scaffolding but must be rebuilt on the Ruff AST before any rule
//! is registered against it.

/// Mutating methods that modify a collection in place.
const MUTATING_METHODS: &[&str] = &[
    "append", "extend", "insert", "remove", "pop", "clear", "sort", "reverse", "update", "add",
    "discard",
];

/// Check a Python source string for ownership violations.
///
/// Detects mutation of `Borrowed` parameters via mutating method calls.
#[must_use]
pub fn check_ownership(source: &str) -> Vec<String> {
    let borrowed_params = collect_borrowed_params(source);
    if borrowed_params.is_empty() {
        return Vec::new();
    }

    source
        .lines()
        .flat_map(|line| {
            let trimmed = line.trim();
            borrowed_params.iter().flat_map(move |param| {
                MUTATING_METHODS
                    .iter()
                    .filter(move |method| trimmed.contains(&format!("{param}.{method}(")))
                    .map(move |method| violation_message(param, method))
            })
        })
        .collect()
}

/// The scaffolding message for one mutation of a borrowed parameter.
fn violation_message(param: &str, method: &str) -> String {
    format!(
        "directives_cast: mutation of Borrowed parameter `{param}` \
         via `.{method}()` is not allowed"
    )
}

/// Extract parameter names annotated with `Borrowed[...]` from source text.
fn collect_borrowed_params(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| parameter_list(line.trim()))
        .flat_map(|params| params.split(',').filter_map(borrowed_parameter_name))
        .map(str::to_owned)
        .collect()
}

/// The text between the outermost parentheses of a single-line `def`, if this
/// line is one.
fn parameter_list(trimmed: &str) -> Option<&str> {
    if !trimmed.starts_with("def ") {
        return None;
    }
    let open = trimmed.find('(')?;
    let close = trimmed.rfind(')')?;
    trimmed.get(open + 1..close)
}

/// The parameter name from one `name: annotation` fragment, when the
/// annotation is `Borrowed` or `Borrowed[...]`.
fn borrowed_parameter_name(part: &str) -> Option<&str> {
    let (name, annotation) = part.trim().split_once(':')?;
    let annotation = annotation.trim();
    (annotation == "Borrowed" || annotation.starts_with("Borrowed[")).then_some(name.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    // [CHKARCH-SAFETY-OWNERSHIP]: a borrowed parameter mutated in place is the
    // violation this scaffolding exists to describe.
    #[test]
    fn mutation_of_a_borrowed_parameter_is_reported() {
        let source = "def foo(x: Borrowed[list]) -> None:\n    x.append(1)\n";
        assert_eq!(
            check_ownership(source),
            vec![
                "directives_cast: mutation of Borrowed parameter `x` via `.append()` is not allowed"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn clean_code_reports_nothing() {
        let source = "def foo(x: int) -> int:\n    return x\n";
        assert_eq!(check_ownership(source), Vec::<String>::new());
    }

    // A bare `Borrowed` annotation binds the same as the subscripted form.
    #[test]
    fn a_bare_borrowed_annotation_is_tracked() {
        let source = "def foo(x: Borrowed) -> None:\n    x.clear()\n";
        assert_eq!(check_ownership(source).len(), 1);
    }

    // Reading a borrowed parameter is allowed; only mutation is a violation.
    #[test]
    fn reading_a_borrowed_parameter_is_allowed() {
        let source = "def foo(x: Borrowed[list]) -> int:\n    return len(x)\n";
        assert_eq!(check_ownership(source), Vec::<String>::new());
    }

    // Every mutating method on every borrowed parameter is reported.
    #[test]
    fn each_mutation_of_each_borrowed_parameter_is_reported() {
        let source =
            "def foo(x: Borrowed[list], y: Borrowed[set]) -> None:\n    x.append(1)\n    y.add(2)\n";
        assert_eq!(check_ownership(source).len(), 2);
    }

    // An unannotated parameter is not borrowed, so mutating it is not a
    // violation — this is the early return in `check_ownership`.
    #[test]
    fn mutating_an_unannotated_parameter_is_not_a_violation() {
        let source = "def foo(x) -> None:\n    x.append(1)\n";
        assert_eq!(check_ownership(source), Vec::<String>::new());
    }

    // A non-`Borrowed` annotation is ignored.
    #[test]
    fn an_owned_parameter_is_not_tracked() {
        let source = "def foo(x: Owned[list]) -> None:\n    x.append(1)\n";
        assert_eq!(check_ownership(source), Vec::<String>::new());
    }

    // Malformed `def` lines must not panic or bind a parameter.
    #[test]
    fn malformed_def_lines_bind_nothing() {
        for source in [
            "def foo\n",
            "def foo(\n",
            "def foo)x: Borrowed[list](\n",
            "def foo()\n",
        ] {
            assert_eq!(
                collect_borrowed_params(source),
                Vec::<String>::new(),
                "malformed def must bind no parameters: {source:?}"
            );
        }
    }

    // A fragment without a `:` carries no annotation to inspect.
    #[test]
    fn an_unannotated_fragment_has_no_borrowed_name() {
        assert_eq!(borrowed_parameter_name("x"), None);
    }

    // Only `def` lines contribute parameters.
    #[test]
    fn non_def_lines_contribute_no_parameters() {
        assert_eq!(
            parameter_list("result = foo(x: Borrowed[list])"),
            None,
            "only a def line declares parameters"
        );
    }
}
