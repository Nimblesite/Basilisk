//! Implements [TYPEINF-ANNOTATION-RESOLUTION] — the checker's **single**
//! annotation entry point. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! An annotation is a *type expression*, and turning it into a type is a
//! name-resolution problem. Every rule that compares a value against a
//! declared type obtains that type here — from the Ruff AST annotation node,
//! resolved through the cascade
//!
//! 1. type-alias table (PEP 695 `type X = ..`, and implicit `X = ..`),
//! 2. same-file class table,
//! 3. import table,
//! 4. builtins,
//! 5. forward reference (a string annotation, parsed and resolved by 1–4),
//!
//! — never by pattern-matching annotation source text. Aliases are
//! *transparent*: `type MyStr = int` behaves exactly as `int` at every
//! nesting depth and regardless of declaration order. A name the cascade
//! cannot resolve is the gradual `Unknown` ([TYPEINF-EXCEEDS-NOUNKNOWN]), so
//! the rule that asked suppresses its diagnostic — silence for what we do not
//! know, never silence for a name we *can* resolve
//! ([#378](https://github.com/Nimblesite/Basilisk/issues/378)).
//!
//! This replaces `InferredType::from_annotation(<source text>)`, which is
//! condemned under [TYPEINF-LEGACY].

mod builtins;
mod forms;
mod index;
mod tables;

use std::cell::RefCell;
use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Operator};

use crate::types::InferredType;

use tables::Tables;

/// Maximum alias-expansion / nesting depth. Beyond it the result is gradual:
/// a bound that terminates evaluation NEVER invents an error.
const MAX_DEPTH: u32 = 32;

/// Resolve one annotation expression against a module — the one-shot form of
/// [`AnnotationResolver`], for callers holding a single annotation.
///
/// Prefer building an [`AnnotationResolver`] when resolving more than one
/// annotation in the same module: the tables are built once there.
#[must_use]
pub fn resolve_annotation(module: &ResolvedModule, expr: &Expr) -> InferredType {
    AnnotationResolver::for_module(module)
        .map_or(InferredType::Unknown, |resolver| resolver.resolve(expr))
}

/// The per-module resolution state: name tables plus an index from annotation
/// spans back to their AST nodes.
#[derive(Debug)]
pub struct AnnotationResolver<'m> {
    tables: Tables<'m>,
    annotations: HashMap<(u32, u32), &'m Expr>,
    /// Memo of every annotation already resolved BY SPAN.
    ///
    /// One annotation is asked about by several rules — a function's return
    /// type is read by the return-compatibility rules and by both narrowing
    /// rules — and evaluating a type expression walks it and allocates the
    /// resulting type. The cascade is pure, so the second answer is the first
    /// one ([CHKARCH-TESTING-BENCH]).
    resolved: RefCell<HashMap<(u32, u32), InferredType>>,
    /// Names of the STRUCTURAL classes this module declares — `Protocol` and
    /// `TypedDict` subclasses, from the resolver's binding-resolved flags
    /// ([RESOLV-CANONICAL-BINDING]). Structural types are satisfied by shape,
    /// not identity, so nominal judgments abstain on them.
    structural: std::collections::HashSet<&'m str>,
    /// The module's binding table ([RESOLV-CANONICAL-BINDING]) — the ONE
    /// lawful answer to "which typing symbol does this expression denote?".
    bindings: &'m basilisk_resolver::BindingTable,
}

/// One step of resolution: the alias parameters currently bound, the aliases
/// being expanded (cycle detection), and the remaining depth budget.
#[derive(Debug, Default, Clone)]
pub(crate) struct Frame {
    bindings: Vec<(String, InferredType)>,
    visiting: Vec<String>,
    depth: u32,
}

impl Frame {
    /// The frame for expanding `alias` with `bindings` bound to its
    /// parameters.
    fn expanding(&self, alias: &str, bindings: Vec<(String, InferredType)>) -> Frame {
        let mut visiting = self.visiting.clone();
        visiting.push(alias.to_owned());
        Frame {
            bindings,
            visiting,
            depth: self.depth + 1,
        }
    }

    /// The same frame one level deeper into a type expression.
    fn nested(&self) -> Frame {
        Frame {
            bindings: self.bindings.clone(),
            visiting: self.visiting.clone(),
            depth: self.depth + 1,
        }
    }
}

impl<'m> AnnotationResolver<'m> {
    /// Build the resolver for a module, parsing its AST through the shared
    /// [`LazyAst`](basilisk_resolver::LazyAst) cache. `None` iff the module
    /// does not parse — parse errors are reported separately.
    #[must_use]
    pub fn for_module(module: &'m ResolvedModule) -> Option<AnnotationResolver<'m>> {
        let parsed = module.lazy_ast.get_or_parse(&module.source, &module.path)?;
        Some(AnnotationResolver {
            tables: Tables::build(&parsed.ast),
            annotations: index::annotation_nodes(&parsed.ast),
            resolved: RefCell::default(),
            structural: module
                .classes
                .iter()
                .filter(|class| class.is_protocol || class.is_typed_dict)
                .map(|class| class.name.as_str())
                .collect(),
            bindings: &module.bindings,
        })
    }

    /// The module's binding table, for callers that resolve typing symbols
    /// on expression nodes ([RESOLV-CANONICAL-BINDING]).
    #[must_use]
    pub fn bindings(&self) -> &basilisk_resolver::BindingTable {
        self.bindings
    }

    /// Does this type mention a STRUCTURAL class this module declares — a
    /// `Protocol` or `TypedDict` — at any depth? Structural targets need
    /// member-level judgment, so nominal assignability rules abstain when
    /// this answers `true`.
    ///
    /// The leaf test is set membership of the engine's `Named` leaf — the
    /// [TYPEINF-LEGACY] boundary — against binding-resolved class nature; no
    /// source text is consulted.
    #[must_use]
    pub fn is_structural_target(&self, ty: &InferredType) -> bool {
        match ty {
            InferredType::Named(name) => self.structural.contains(name.as_str()),
            InferredType::Union(arms) => arms.iter().any(|arm| self.is_structural_target(arm)),
            InferredType::Optional(inner)
            | InferredType::List(inner)
            | InferredType::Set(inner)
            | InferredType::TypeForm(inner) => self.is_structural_target(inner),
            InferredType::Dict(key, value) => {
                self.is_structural_target(key) || self.is_structural_target(value)
            }
            InferredType::Tuple(elements) => {
                elements.iter().any(|element| self.is_structural_target(element))
            }
            _ => false,
        }
    }

    /// Resolve an annotation expression to the type it denotes.
    ///
    /// NOT memoized: [`Self::resolve_text`] routes standalone-parsed
    /// expressions through here, and their ranges all start at zero — a
    /// range-keyed cache would let one text's answer masquerade as
    /// another's. Only [`Self::resolve_span`], whose keys are module-anchored
    /// annotation nodes, caches.
    #[must_use]
    pub fn resolve(&self, expr: &Expr) -> InferredType {
        self.eval(expr, &Frame::default())
    }

    /// Resolve the annotation node covering `span`. `None` when no annotation
    /// node has exactly that span — the caller then has no annotation to judge
    /// and must stay silent rather than fall back to reading text.
    #[must_use]
    pub fn resolve_span(&self, span: Span) -> Option<InferredType> {
        let key = (span.start, span.end);
        if let Some(hit) = self.resolved.borrow().get(&key) {
            return Some(hit.clone());
        }
        let resolved = self.resolve(self.annotations.get(&key)?);
        let _ = self.resolved.borrow_mut().insert(key, resolved.clone());
        Some(resolved)
    }

    /// Resolve an annotation the resolver holds only as **stored text** — a
    /// `ResolvedModule` field that kept the annotation's rendering but not its
    /// span.
    ///
    /// The text is parsed by `ruff` into the type expression it always was and
    /// then run through this same cascade, so the caller gets alias expansion,
    /// same-file classes and shadowing exactly as `resolve_span` does. It is
    /// *not* the condemned text path: nothing here pattern-matches source
    /// characters. `None` when the text is not a parseable type expression.
    ///
    /// Callers that can reach the annotation node should use [`Self::resolve`]
    /// or [`Self::resolve_span`]; this seam closes as the resolver's structures
    /// grow spans ([NARROWPLAN-INTEGRATION]).
    #[must_use]
    pub fn resolve_text(&self, text: &str) -> Option<InferredType> {
        let parsed = ruff_python_parser::parse_expression(text.trim()).ok()?;
        Some(self.resolve(parsed.expr()))
    }

    /// Is `name` a leaf the module GROUNDS — a class declared here or a
    /// builtin type? An unresolved spelling (a `TypeVar`, an imported class
    /// this module cannot see into, a typo) is NOT grounded, and a judgment
    /// that needs to know what the name IS must abstain rather than guess.
    #[must_use]
    pub fn is_grounded_name(&self, name: &str) -> bool {
        let base = name.split('[').next().unwrap_or(name);
        self.tables.nominal.contains(base) || builtins::is_builtin_type_name(base)
    }

    /// The cascade over one type expression.
    pub(crate) fn eval(&self, expr: &Expr, frame: &Frame) -> InferredType {
        if frame.depth > MAX_DEPTH {
            return InferredType::Unknown;
        }
        match expr {
            Expr::Name(name) => self.name(name.id.as_str(), frame),
            Expr::Attribute(_) => self.attribute(expr, frame),
            Expr::Subscript(sub) => self.subscript(sub, frame),
            Expr::BinOp(bin) if bin.op == Operator::BitOr => self.union(bin, frame),
            Expr::NoneLiteral(_) => InferredType::None_,
            Expr::StringLiteral(text) => self.forward_ref(text.value.to_str(), frame),
            Expr::Starred(star) => forms::unpacked_marker(&self.eval(&star.value, frame)),
            // The `tuple[X, ...]` / `Callable[..., R]` terminator is a
            // structural marker the assignability judgment reads.
            Expr::EllipsisLiteral(_) => InferredType::Named("...".to_owned()),
            _ => InferredType::Unknown,
        }
    }

    /// A bare name, in cascade order: alias parameters bound by an enclosing
    /// expansion, then aliases, classes, imports, and builtins last — a
    /// module-level declaration shadows a builtin exactly as Python does.
    fn name(&self, name: &str, frame: &Frame) -> InferredType {
        if let Some((_, bound)) = frame.bindings.iter().find(|(param, _)| param == name) {
            return bound.clone();
        }
        if let Some(expanded) = self.expand_alias(name, &[], frame) {
            return expanded;
        }
        if self.tables.nominal.contains(name) {
            return InferredType::Named(name.to_owned());
        }
        if let Some(imported) = self.imported_leaf(name) {
            return imported;
        }
        builtins::leaf(name).unwrap_or(InferredType::Unknown)
    }

    /// A dotted name: `mod.Class`.
    fn attribute(&self, expr: &Expr, frame: &Frame) -> InferredType {
        let Some(dotted) = tables::dotted_name(expr) else {
            return InferredType::Unknown;
        };
        let Some(head) = self.canonical_head(&dotted) else {
            return InferredType::Unknown;
        };
        match self.name(&head, frame) {
            // `canonical_head` only yields a member for a module the cascade
            // knows, so a member with no modelled form is still a name it
            // resolved — nominal, not gradual (see [`Self::imported_leaf`]).
            InferredType::Unknown => InferredType::Named(head),
            resolved => resolved,
        }
    }

    /// A subscripted form: special forms first, then parameterised aliases,
    /// then generic same-file classes.
    fn subscript(&self, sub: &ruff_python_ast::ExprSubscript, frame: &Frame) -> InferredType {
        let args = basilisk_parser::subscript_elements(sub);
        let Some(head) = tables::dotted_name(&sub.value).and_then(|d| self.canonical_head(&d))
        else {
            return InferredType::Unknown;
        };
        let nested = frame.nested();
        if !self.shadows_special_form(&head) {
            if let Some(ty) = forms::special_form(self, &head, &args, &nested) {
                return ty;
            }
        }
        if let Some(expanded) = self.expand_alias(&head, &args, frame) {
            return expanded;
        }
        if self.tables.nominal.contains(&head) {
            return InferredType::Named(head);
        }
        InferredType::Unknown
    }

    /// `X | Y` — flattened into one union.
    fn union(&self, bin: &ruff_python_ast::ExprBinOp, frame: &Frame) -> InferredType {
        let mut arms = Vec::new();
        self.union_arm(&bin.left, frame, &mut arms);
        self.union_arm(&bin.right, frame, &mut arms);
        InferredType::Union(arms)
    }

    fn union_arm(&self, expr: &Expr, frame: &Frame, arms: &mut Vec<InferredType>) {
        match expr {
            Expr::BinOp(bin) if bin.op == Operator::BitOr => {
                self.union_arm(&bin.left, frame, arms);
                self.union_arm(&bin.right, frame, arms);
            }
            other => arms.push(self.eval(other, frame)),
        }
    }

    /// A string annotation is a forward reference: parse it and resolve the
    /// expression it contains through this same cascade.
    fn forward_ref(&self, text: &str, frame: &Frame) -> InferredType {
        match ruff_python_parser::parse_expression(text.trim()) {
            Ok(parsed) => self.eval(parsed.expr(), &frame.nested()),
            Err(_) => InferredType::Unknown,
        }
    }

    /// Expand an alias transparently, binding its parameters to the resolved
    /// arguments. A cycle (`type J = list[J]` re-entered) is gradual, which
    /// terminates expansion without rejecting the legal recursive alias
    /// ([#371](https://github.com/Nimblesite/Basilisk/issues/371)).
    ///
    /// The cut MUST stay gradual. Cutting to `Named(alias)` instead — to keep
    /// the self-reference visible for a consumer that wants to keep matching —
    /// was tried and reverted: `Named` is not accepting in
    /// `is_assignable_to`, so `type A[T] = T | list[A[T]]` stopped accepting
    /// `[1, [1, 2, 3]]` (`no_false_positive_on_pep695_type_alias_annotation`).
    /// A consumer that needs the self-reference needs the alias body's own
    /// shape, not a differently-cut expansion ([NARROWPLAN-INTEGRATION]
    /// Step 7).
    fn expand_alias(&self, name: &str, args: &[&Expr], frame: &Frame) -> Option<InferredType> {
        let entry = self.tables.aliases.get(name)?;
        if frame.visiting.iter().any(|visited| visited == name) {
            return Some(InferredType::Unknown);
        }
        let bindings = entry
            .params
            .iter()
            .cloned()
            .zip(args.iter().map(|arg| self.eval(arg, &frame.nested())))
            .collect();
        Some(self.eval(entry.value, &frame.expanding(name, bindings)))
    }

    /// A name bound by a `from`-import of a typing module resolves to that
    /// member as a **nominal** type: it is a name the cascade *did* resolve,
    /// and calling it gradual would silence judgments the nominal comparison
    /// can still make
    /// ([#378](https://github.com/Nimblesite/Basilisk/issues/378)). What the
    /// member MEANS is not decided here and must not be: that is a question
    /// about its declaration, and the mechanism that answers it lawfully does
    /// not exist yet. Project and third-party symbols stay gradual until the
    /// import cascade covers them — the seam
    /// [#324](https://github.com/Nimblesite/Basilisk/issues/324) fills, behind
    /// this same entry point.
    fn imported_leaf(&self, name: &str) -> Option<InferredType> {
        let imported = self.tables.imports.get(name)?;
        if !builtins::is_typing_module(&imported.module) {
            return Some(InferredType::Unknown);
        }
        Some(InferredType::Named(imported.original.clone()))
    }

    /// Rewrite a spelling into the name the cascade knows it by: an
    /// import alias becomes the name as spelled in its defining module, and a
    /// `typing`-qualified attribute becomes its bare member name.
    fn canonical_head(&self, dotted: &str) -> Option<String> {
        let Some((head, member)) = dotted.split_once('.') else {
            return Some(
                self.tables
                    .imports
                    .get(dotted)
                    .filter(|imported| builtins::is_typing_module(&imported.module))
                    .map_or_else(|| dotted.to_owned(), |imported| imported.original.clone()),
            );
        };
        let module = self.tables.modules.get(head)?;
        builtins::is_typing_module(module).then(|| member.to_owned())
    }

    /// A module-level declaration of the same name wins over a special form:
    /// a class declared in this file means *this* class.
    fn shadows_special_form(&self, head: &str) -> bool {
        self.tables.nominal.contains(head) || self.tables.aliases.contains_key(head)
    }
}
