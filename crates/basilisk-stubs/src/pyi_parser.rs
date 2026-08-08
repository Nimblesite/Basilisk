//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! `.pyi` stub file parser.
//!
//! Parses `.pyi` files using `basilisk-parser` and extracts structured
//! type information into a [`StubModule`]. Handles `@overload` grouping,
//! class definitions with methods and attributes, and module-level
//! variable annotations.

mod class_members;
mod dunder_all;
mod guard;
mod imports;
mod syntax;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_canonical::{BindingTable, TypingForm};

use ruff_python_ast::{
    Expr, Operator, Stmt, StmtAnnAssign, StmtAssign, StmtAugAssign, StmtFunctionDef, StmtIf,
};

use crate::types::{
    DunderAllItem, DunderAllMutation, StarReexport, StubClass, StubFunction, StubModule,
    StubSource, StubSpan, StubTarget, StubTier, StubVariable,
};

pub(crate) use self::dunder_all::intersect_name_lists;
use self::dunder_all::{
    dotted_expression, literal_dunder_all, literal_dunder_all_items, string_literal,
};
use self::guard::feasible_branches;
use self::syntax::{
    ann_assign_target_name, expr_to_annotation, extract_decorator_names, extract_params,
    has_decorator_form,
};

/// Errors that can occur during `.pyi` parsing.
#[derive(Debug, thiserror::Error)]
pub enum StubParseError {
    /// Failed to read the `.pyi` file from disk.
    #[error("failed to read stub file {path}: {source}")]
    Io {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The `.pyi` file contained syntax errors.
    #[error("syntax error in stub file {path}: {message}")]
    Syntax {
        /// Path with the syntax error.
        path: PathBuf,
        /// Error description.
        message: String,
    },
}

/// Parse a `.pyi` file from disk and extract a [`StubModule`].
///
/// # Errors
///
/// Returns [`StubParseError::Io`] if the file cannot be read, or
/// [`StubParseError::Syntax`] if the file has parse errors.
pub fn parse_pyi_file(
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
) -> Result<StubModule, StubParseError> {
    // Routed through `read_tracked` so the result cache records stub reads too.
    // See [CHKCACHE-READSET-FS].
    let content = basilisk_common::fs::read_tracked(path).map_err(|err| StubParseError::Io {
        path: path.to_path_buf(),
        source: err,
    })?;
    parse_pyi_source_with_target(&content, path, module_name, source, tier, None)
}

/// Parse a `.pyi` file using concrete target evidence for guarded declarations.
///
/// # Errors
///
/// Returns [`StubParseError::Io`] if the file cannot be read, or
/// [`StubParseError::Syntax`] if the file has parse errors.
pub fn parse_pyi_file_for_target(
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
    target: &StubTarget,
) -> Result<StubModule, StubParseError> {
    let content = basilisk_common::fs::read_tracked(path).map_err(|err| StubParseError::Io {
        path: path.to_path_buf(),
        source: err,
    })?;
    parse_pyi_source_with_target(
        &content,
        path,
        module_name,
        source,
        tier,
        Some(target.clone()),
    )
}

/// Parse `.pyi` source text and extract a [`StubModule`].
///
/// # Errors
///
/// Returns [`StubParseError::Syntax`] if the source has parse errors.
// Implements [STUBRES-PYI] — reuses `basilisk-parser`; only signatures
// matter (def/class/annotations), bodies (`...`/`pass`) are ignored, and the
// `@overload` decorator is tracked (see `StubExtractor::visit_function`).
pub fn parse_pyi_source(
    content: &str,
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
) -> Result<StubModule, StubParseError> {
    parse_pyi_source_with_target(content, path, module_name, source, tier, None)
}

/// Parse `.pyi` source using concrete target evidence for guarded declarations.
///
/// # Errors
///
/// Returns [`StubParseError::Syntax`] if the source has parse errors.
pub fn parse_pyi_source_for_target(
    content: &str,
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
    target: &StubTarget,
) -> Result<StubModule, StubParseError> {
    parse_pyi_source_with_target(
        content,
        path,
        module_name,
        source,
        tier,
        Some(target.clone()),
    )
}

/// Every string literal a `sys.platform` guard in this stub tests against.
///
/// A stub's extracted shape depends on the target platform ONLY through these
/// comparisons, so this is the complete set of platform values that can change
/// the result — everything else falls in one indistinguishable class. The
/// precomputed builtins index uses it to enumerate a provably complete set of
/// platform variants ([STUBRES-TYPESHED-BUILTINS-INDEX]).
///
/// # Errors
///
/// Returns [`StubParseError::Syntax`] if the source has parse errors.
pub fn platform_guard_literals(
    content: &str,
    path: &Path,
) -> Result<std::collections::BTreeSet<String>, StubParseError> {
    let module_ast =
        basilisk_parser::parse_source(content.to_owned(), path.to_string_lossy().into_owned())
            .map_err(|err| StubParseError::Syntax {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?
            .ast;
    Ok(guard::platform_guard_literals(&module_ast.body))
}

fn parse_pyi_source_with_target(
    content: &str,
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
    target: Option<StubTarget>,
) -> Result<StubModule, StubParseError> {
    let module_ast =
        basilisk_parser::parse_source(content.to_owned(), path.to_string_lossy().into_owned())
            .map_err(|err| StubParseError::Syntax {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?
            .ast;
    // One table for the whole module: `BindingTable::from_module` already
    // descends into `if TYPE_CHECKING:` and `try`/`except ImportError` bodies,
    // so branch visiting never needs a different view of the imports.
    let bindings = Arc::new(BindingTable::from_module(&module_ast.body));
    let mut extractor = StubExtractor::new(module_name, path, source, tier, target, bindings);
    extractor.visit_body(&module_ast.body);
    Ok(extractor.into_module())
}

/// Walks the AST and collects stub information.
///
/// The large collections hold their values behind `Arc` so that the guard
/// intersection in [`Self::visit_if`] can clone the whole accumulated state
/// per feasible branch as refcount bumps: entries a branch never touches stay
/// pointer-identical across alternatives, making both the clone and the
/// intersection O(branch effects) instead of O(module so far) — the
/// difference between milliseconds and tens of milliseconds on
/// `builtins.pyi` with no version target. [`Self::into_module`] unwraps the
/// `Arc`s (refcount is back to one by then), so [`StubModule`]'s public shape
/// is unchanged.
#[derive(Clone)]
struct StubExtractor {
    module_name: String,
    path: PathBuf,
    source: StubSource,
    tier: StubTier,
    target: Option<StubTarget>,
    functions: HashMap<String, Arc<StubFunction>>,
    overloads: HashMap<String, Arc<Vec<StubFunction>>>,
    classes: HashMap<String, Arc<StubClass>>,
    variables: HashMap<String, Arc<StubVariable>>,
    dunder_all_mutations: Vec<DunderAllMutation>,
    reexported_names: Vec<String>,
    star_reexports: Vec<StarReexport>,
    module_bindings: HashMap<String, StarReexport>,
    /// What each name in this stub refers to. Shared behind `Arc` so the
    /// per-branch clones in [`Self::visit_if`] stay refcount bumps.
    bindings: Arc<BindingTable>,
}

impl StubExtractor {
    fn new(
        module_name: &str,
        path: &Path,
        source: StubSource,
        tier: StubTier,
        target: Option<StubTarget>,
        bindings: Arc<BindingTable>,
    ) -> Self {
        Self {
            module_name: module_name.to_owned(),
            path: path.to_path_buf(),
            source,
            tier,
            target,
            functions: HashMap::new(),
            overloads: HashMap::new(),
            classes: HashMap::new(),
            variables: HashMap::new(),
            dunder_all_mutations: Vec::new(),
            reexported_names: Vec::new(),
            star_reexports: Vec::new(),
            module_bindings: HashMap::new(),
            bindings,
        }
    }

    fn into_module(self) -> StubModule {
        StubModule {
            module_name: self.module_name,
            path: self.path,
            source: self.source,
            tier: self.tier,
            target: self.target,
            functions: unwrap_shared_map(self.functions),
            overloads: unwrap_shared_map(self.overloads),
            classes: unwrap_shared_map(self.classes),
            variables: unwrap_shared_map(self.variables),
            dunder_all: literal_dunder_all(&self.dunder_all_mutations),
            dunder_all_mutations: self.dunder_all_mutations,
            reexported_names: self.reexported_names,
            star_reexports: self.star_reexports,
        }
    }

    /// Visit all top-level statements in a module body.
    fn visit_body(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => self.visit_function(func, None),
                Stmt::ClassDef(class) => self.visit_class(class),
                Stmt::AnnAssign(ann) => self.visit_ann_assign(ann),
                Stmt::Assign(assign) => self.visit_assign(assign),
                Stmt::AugAssign(aug) => self.visit_aug_assign(aug),
                Stmt::Expr(expr) => self.visit_expr(&expr.value),
                Stmt::Import(import) => self.visit_import(import),
                Stmt::ImportFrom(import) => self.visit_import_from(import),
                Stmt::If(if_stmt) => self.visit_if(if_stmt),
                _ => {}
            }
        }
    }

    /// Select concrete target branches, or intersect all feasible alternatives
    /// when platform/version evidence is absent or explicitly `All`.
    ///
    /// Each alternative clones `self`, but the maps share values behind `Arc`,
    /// so the clone is a table copy plus refcount bumps and the intersection
    /// short-circuits untouched entries by pointer identity.
    fn visit_if(&mut self, if_stmt: &StmtIf) {
        let branches = feasible_branches(if_stmt, self.target.as_ref());
        if branches.len() == 1 {
            if let Some(body) = branches.first().and_then(|body| *body) {
                self.visit_body(body);
            }
            return;
        }

        let mutation_prefix_len = self.dunder_all_mutations.len();
        let alternatives: Vec<Self> = branches
            .into_iter()
            .map(|body| {
                let mut alternative = self.clone();
                if let Some(stmts) = body {
                    alternative.visit_body(stmts);
                }
                alternative
            })
            .collect();
        let branch_mutations: Vec<Vec<DunderAllMutation>> = alternatives
            .iter()
            .map(|alternative| {
                alternative
                    .dunder_all_mutations
                    .get(mutation_prefix_len..)
                    .map_or_else(Vec::new, <[DunderAllMutation]>::to_vec)
            })
            .collect();
        if let Some(mut intersection) = Self::intersect_alternatives(&alternatives) {
            intersection
                .dunder_all_mutations
                .truncate(mutation_prefix_len);
            if branch_mutations.iter().any(|branch| !branch.is_empty()) {
                intersection
                    .dunder_all_mutations
                    .push(DunderAllMutation::Choice(branch_mutations));
            }
            *self = intersection;
        }
    }

    fn intersect_alternatives(alternatives: &[Self]) -> Option<Self> {
        let mut intersection = alternatives.first()?.clone();
        for alternative in alternatives.iter().skip(1) {
            retain_matching_entries(
                &mut intersection.functions,
                &alternative.functions,
                |left, right| Arc::ptr_eq(left, right) || same_stub_function(left, right),
            );
            union_common_overloads(&mut intersection.overloads, &alternative.overloads);
            retain_matching_entries(
                &mut intersection.classes,
                &alternative.classes,
                |left, right| Arc::ptr_eq(left, right) || same_stub_class(left, right),
            );
            retain_matching_entries(
                &mut intersection.variables,
                &alternative.variables,
                |left, right| Arc::ptr_eq(left, right) || left == right,
            );
            retain_equal_entries(
                &mut intersection.module_bindings,
                &alternative.module_bindings,
            );
            retain_common(
                &mut intersection.reexported_names,
                &alternative.reexported_names,
            );
            retain_common(
                &mut intersection.star_reexports,
                &alternative.star_reexports,
            );
        }
        Some(intersection)
    }

    fn visit_expr(&mut self, expr: &Expr) {
        let Expr::Call(call) = expr else {
            return;
        };
        let Expr::Attribute(method) = call.func.as_ref() else {
            return;
        };
        if !matches!(method.value.as_ref(), Expr::Name(name) if name.id == "__all__")
            || !call.arguments.keywords.is_empty()
            || call.arguments.args.len() != 1
        {
            return;
        }
        let Some(argument) = call.arguments.args.first() else {
            return;
        };
        match method.attr.as_str() {
            "extend" => {
                if let Some(items) = self.dunder_all_items(argument) {
                    self.record_dunder_all_mutation(DunderAllMutation::Extend(items));
                }
            }
            "append" => {
                if let Some(name) = string_literal(argument) {
                    self.record_dunder_all_mutation(DunderAllMutation::Append(name));
                }
            }
            "remove" => {
                if let Some(name) = string_literal(argument) {
                    self.record_dunder_all_mutation(DunderAllMutation::Remove(name));
                }
            }
            _ => {}
        }
    }

    /// Record `__all__ = [...]` / `__all__ = (...)` entries.
    fn collect_assign_dunder_all(&mut self, assign: &StmtAssign) {
        let targets_all = assign
            .targets
            .iter()
            .any(|target| matches!(target, Expr::Name(name) if name.id == "__all__"));
        if targets_all {
            if let Some(items) = self.dunder_all_items(&assign.value) {
                self.record_dunder_all_mutation(DunderAllMutation::Assign(items));
            }
        }
    }

    /// Record `__all__ += [...]` extensions ([STUBRES-PYI-REEXPORTS]).
    fn visit_aug_assign(&mut self, aug: &StmtAugAssign) {
        if matches!(aug.target.as_ref(), Expr::Name(name) if name.id == "__all__")
            && matches!(aug.op, Operator::Add)
        {
            if let Some(items) = self.dunder_all_items(&aug.value) {
                self.record_dunder_all_mutation(DunderAllMutation::Extend(items));
            }
        }
    }

    fn record_dunder_all_mutation(&mut self, mutation: DunderAllMutation) {
        self.dunder_all_mutations.push(mutation);
    }

    fn dunder_all_items(&self, value: &Expr) -> Option<Vec<DunderAllItem>> {
        match value {
            Expr::List(list) => literal_dunder_all_items(&list.elts),
            Expr::Tuple(tuple) => literal_dunder_all_items(&tuple.elts),
            Expr::BinOp(binary) if matches!(binary.op, Operator::Add) => {
                let mut items = self.dunder_all_items(&binary.left)?;
                items.extend(self.dunder_all_items(&binary.right)?);
                Some(items)
            }
            Expr::Name(name) => self
                .module_bindings
                .get(name.id.as_str())
                .cloned()
                .map(DunderAllItem::ModuleAll)
                .map(|item| vec![item]),
            Expr::Attribute(_) => self
                .module_all_reference(value)
                .map(DunderAllItem::ModuleAll)
                .map(|item| vec![item]),
            _ => None,
        }
    }

    fn module_all_reference(&self, value: &Expr) -> Option<StarReexport> {
        let dotted = dotted_expression(value)?;
        let mut segments = dotted.split('.');
        let binding = segments.next()?;
        let suffix: Vec<&str> = segments.collect();
        if suffix.last().copied() != Some("__all__") {
            return None;
        }
        let mut target = self.module_bindings.get(binding)?.clone();
        let module_suffix = suffix.get(..suffix.len().saturating_sub(1))?.join(".");
        if !module_suffix.is_empty() {
            target.module = if target.module.is_empty() {
                module_suffix
            } else {
                format!("{}.{}", target.module, module_suffix)
            };
        }
        Some(target)
    }

    fn visit_function(&mut self, func: &StmtFunctionDef, class_name: Option<&str>) {
        let decorators = extract_decorator_names(&func.decorator_list);
        let params = extract_params(&func.parameters);
        let return_type = func.returns.as_ref().map(|ret| expr_to_annotation(ret));

        let stub_fn = StubFunction {
            name: func.name.to_string(),
            receiver: None,
            params,
            return_type,
            is_async: func.is_async,
            is_overload: has_decorator_form(
                &self.bindings,
                &func.decorator_list,
                TypingForm::Overload,
            ),
            decorators,
            class_name: class_name.map(str::to_owned),
            source_span: StubSpan {
                start: func.name.range.start().into(),
                end: func.name.range.end().into(),
            },
        };

        let name = func.name.to_string();
        if stub_fn.is_overload {
            Arc::make_mut(self.overloads.entry(name).or_default()).push(stub_fn);
        } else {
            let _ = self.functions.insert(name, Arc::new(stub_fn));
        }
    }

    fn visit_ann_assign(&mut self, ann: &StmtAnnAssign) {
        if let Some(name) = ann_assign_target_name(ann) {
            let _ = self.variables.insert(
                name.clone(),
                Arc::new(StubVariable {
                    name,
                    annotation: Some(expr_to_annotation(&ann.annotation)),
                }),
            );
        }
    }

    fn visit_assign(&mut self, assign: &StmtAssign) {
        self.collect_assign_dunder_all(assign);
        // Handle `__all__ = [...]` and type alias assignments like `X = int`.
        for target in &assign.targets {
            if let Expr::Name(name_expr) = target {
                let annotation = Some(expr_to_annotation(&assign.value));
                let _ = self.variables.insert(
                    name_expr.id.to_string(),
                    Arc::new(StubVariable {
                        name: name_expr.id.to_string(),
                        annotation,
                    }),
                );
            }
        }
    }
}

/// Unwrap an `Arc`-shared map into its plain form. By the time a module is
/// finalized every intersection alternative has been dropped, so each `Arc`
/// is unique and unwrapping moves the value without a deep clone.
fn unwrap_shared_map<K, V>(map: HashMap<K, Arc<V>>) -> HashMap<K, V>
where
    K: std::hash::Hash + Eq,
    V: Clone,
{
    map.into_iter()
        .map(|(key, value)| (key, Arc::unwrap_or_clone(value)))
        .collect()
}

fn retain_equal_entries<K, V>(left: &mut HashMap<K, V>, right: &HashMap<K, V>)
where
    K: std::hash::Hash + Eq,
    V: Eq,
{
    left.retain(|key, value| right.get(key) == Some(value));
}

fn retain_matching_entries<K, V, F>(left: &mut HashMap<K, V>, right: &HashMap<K, V>, matches: F)
where
    K: std::hash::Hash + Eq,
    F: Fn(&V, &V) -> bool,
{
    left.retain(|key, value| right.get(key).is_some_and(|other| matches(value, other)));
}

pub(super) fn retain_common<T: PartialEq>(left: &mut Vec<T>, right: &[T]) {
    left.retain(|value| right.contains(value));
}

pub(super) fn retain_common_by<T>(left: &mut Vec<T>, right: &[T], matches: fn(&T, &T) -> bool) {
    left.retain(|value| right.iter().any(|other| matches(value, other)));
}

pub(super) fn same_stub_function(left: &StubFunction, right: &StubFunction) -> bool {
    left.name == right.name
        && left.receiver == right.receiver
        && left.params == right.params
        && left.return_type == right.return_type
        && left.is_overload == right.is_overload
        && left.is_async == right.is_async
        && left.decorators == right.decorators
        && left.class_name == right.class_name
}

/// Intersect overload groups by NAME across feasible branches, unioning the
/// variants of a name that appears in every branch.
///
/// A plain [`retain_matching_entries`] keyed on `same_stub_functions` drops an
/// overload whose variant list differs across a version gate — but a function
/// like `ast.parse`, declared under BOTH arms of `if sys.version_info >= (3,
/// 15)`, exists regardless of the (unknown) target version. Dropping it reported
/// `ast.parse` as a missing attribute (GitHub #324). Keeping the name and
/// unioning the per-branch variants preserves membership without inventing a
/// declaration no branch made. A name present in only SOME branches is still
/// intersected away — it may genuinely not exist on the resolved version.
fn union_common_overloads(
    left: &mut HashMap<String, Arc<Vec<StubFunction>>>,
    right: &HashMap<String, Arc<Vec<StubFunction>>>,
) {
    left.retain(|name, variants| match right.get(name) {
        // Pointer-identical groups were untouched by every branch — nothing
        // to union.
        Some(other) if Arc::ptr_eq(variants, other) => true,
        Some(other) => {
            for variant in other.iter() {
                if !variants
                    .iter()
                    .any(|existing| same_stub_function(existing, variant))
                {
                    Arc::make_mut(variants).push(variant.clone());
                }
            }
            true
        }
        None => false,
    });
}

fn same_stub_functions(left: &[StubFunction], right: &[StubFunction]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same_stub_function(left, right))
}

fn same_stub_class(left: &StubClass, right: &StubClass) -> bool {
    left.name == right.name
        && left.bases == right.bases
        && left.metaclass == right.metaclass
        && same_stub_functions(&left.methods, &right.methods)
        && left.attributes == right.attributes
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixed parser fixture must fail loudly"
)]
mod regression_tests {
    use std::fmt::Write as _;
    use std::path::Path;

    use super::parse_pyi_source;
    use crate::types::{StubSource, StubTier};

    /// Unknown-guard intersection semantics ([STUBRES-PYI]): a symbol exists
    /// only when EVERY feasible branch agrees on it; symbols untouched by the
    /// guard survive unchanged. Pins the behavior the branch-sharing
    /// optimization in `visit_if` must preserve.
    #[test]
    fn unknown_guard_intersection_keeps_agreeing_and_drops_diverging_symbols() {
        let module = parse_pyi_source(
            "def stable() -> int: ...\n\
             x: int\n\
             if feature:\n    def gated() -> int: ...\n    def stable() -> str: ...\n\
             else:\n    def gated() -> int: ...\n",
            Path::new("intersect.pyi"),
            "intersect",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert!(
            module.functions.contains_key("gated"),
            "identical declarations under every arm exist regardless of the guard"
        );
        assert!(
            !module.functions.contains_key("stable"),
            "a symbol the arms disagree on is intersected away"
        );
        assert!(
            module.variables.contains_key("x"),
            "symbols untouched by the guard survive"
        );
    }

    /// A class member added under an unknown guard survives only when the
    /// class already declares it identically before the guard — intersecting
    /// `pre + additions` with the untouched `pre` (including the preserved
    /// duplicate). Pins the clone-free class-guard fast path.
    #[test]
    fn class_guard_additions_survive_only_when_already_declared() {
        let module = parse_pyi_source(
            "class C:\n\
             \x20   def stable(self) -> int: ...\n\
             \x20   if feature:\n\
             \x20       def stable(self) -> int: ...\n\
             \x20       def gated(self) -> int: ...\n",
            Path::new("class_guard.pyi"),
            "class_guard",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        let class = module.classes.get("C").expect("class C extracted");
        assert_eq!(
            class
                .methods
                .iter()
                .filter(|method| method.name == "stable")
                .count(),
            2,
            "an identical guarded redeclaration is preserved alongside the original"
        );
        assert!(
            !class.methods.iter().any(|method| method.name == "gated"),
            "a guarded-only member is not guaranteed to exist"
        );
    }

    /// Nested unknown guards intersect through their enclosing branches: a
    /// symbol reachable identically on every path exists; one missing from any
    /// path does not.
    #[test]
    fn nested_unknown_guards_intersect_through_outer_branches() {
        let module = parse_pyi_source(
            "def outer() -> int: ...\n\
             if feature_a:\n\
             \x20   if feature_b:\n        def inner() -> int: ...\n\
             \x20   else:\n        def inner() -> int: ...\n\
             \x20   def only_a() -> int: ...\n\
             else:\n    def inner() -> int: ...\n",
            Path::new("nested.pyi"),
            "nested",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert!(
            module.functions.contains_key("inner"),
            "declared identically on every feasible path"
        );
        assert!(
            !module.functions.contains_key("only_a"),
            "absent from the else path, so not guaranteed to exist"
        );
        assert!(module.functions.contains_key("outer"));
    }

    #[test]
    fn independent_unknown_guards_do_not_duplicate_identical_all_histories() {
        let mut source = String::from("__all__ = ['public']\n");
        for index in 0..32 {
            let _ = write!(source, "if feature_{index}:\n    value_{index}: int\n");
        }
        let module = parse_pyi_source(
            &source,
            Path::new("guards.pyi"),
            "guards",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert_eq!(module.dunder_all_mutations.len(), 1);
        assert_eq!(module.dunder_all, Some(vec!["public".to_owned()]));
    }

    #[test]
    fn guarded_all_mutations_do_not_create_a_power_set() {
        let mut source = String::from("__all__ = ['base']\n");
        for index in 0..8 {
            let _ = write!(
                source,
                "if feature_{index}:\n    __all__.append('optional_{index}')\n"
            );
        }
        let module = parse_pyi_source(
            &source,
            Path::new("guarded_all.pyi"),
            "guarded_all",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert_eq!(module.dunder_all_mutations.len(), 9);
        assert_eq!(module.dunder_all, Some(vec!["base".to_owned()]));
    }

    #[test]
    fn guarded_all_meet_preserves_duplicate_counts_for_later_remove() {
        let module = parse_pyi_source(
            "__all__ = ['shared']\nif feature:\n    __all__.append('shared')\nelse:\n    __all__.extend(['shared', 'shared'])\n__all__.remove('shared')\n",
            Path::new("guarded_counts.pyi"),
            "guarded_counts",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert_eq!(module.dunder_all, Some(vec!["shared".to_owned()]));
    }

    #[test]
    fn identical_guarded_append_is_visible_to_a_later_remove() {
        let module = parse_pyi_source(
            "if feature:\n    __all__.append('shared')\nelse:\n    __all__.append('shared')\n__all__.remove('shared')\n",
            Path::new("guarded_remove.pyi"),
            "guarded_remove",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses");
        assert_eq!(module.dunder_all, Some(Vec::new()));
    }
}
