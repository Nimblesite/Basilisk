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
    /// The module's binding table ([RESOLV-CANONICAL-BINDING]) — the ONE
    /// lawful answer to "which typing symbol does this expression denote?".
    bindings: &'m basilisk_resolver::BindingTable,
    /// The module's classes, for grounding a member leaf (`Color.RED`)
    /// against the declarations of the class its qualifier resolves to.
    ///
    /// ORPHANED BY A DELETION, NOT UNUSED. Its only reader was
    /// `class_declares`, reached from `grounds`, whose caller
    /// `is_grounded_name` was deleted for taking a `&str`. Both are retained
    /// as the rebuild map — see their banners.
    #[expect(
        dead_code,
        reason = "reader orphaned by the `is_grounded_name` deletion; retained for the \
                  rebuild that passes an `Expr` instead of a rendering"
    )]
    classes: &'m [basilisk_resolver::ClassInfo],
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
            bindings: &module.bindings,
            classes: &module.classes,
        })
    }

    /// The module's binding table, for callers that resolve typing symbols
    /// on expression nodes ([RESOLV-CANONICAL-BINDING]).
    #[must_use]
    pub fn bindings(&self) -> &basilisk_resolver::BindingTable {
        self.bindings
    }

    // ######################################################################
    // # DELETED BODY — `is_structural_target`. DO NOT RESTORE IT.           #
    // #                                                                     #
    // # The resolver knew which class DEFINITIONS were Protocols and        #
    // # TypedDicts, but this function discarded those identities into a set #
    // # of class-name strings, then classified every `Named(String)` leaf by #
    // # membership of that spelling:                                        #
    // #                                                                     #
    // #   InferredType::Named(name) => structural.contains(name.as_str())   #
    // #                                                                     #
    // # Two same-spelled definitions therefore became one structural type, #
    // # while an alias of a structural class was not one. Recursing through #
    // # containers only propagated the bad leaf verdict more deeply.        #
    // #                                                                     #
    // # The rebuild requires nominal leaves to carry the definition site    #
    // # resolved from their original AST node. Structural classification is #
    // # then a property of that definition, never of its display name.      #
    // #                                                                     #
    // # Pinned by: tests/nominal_leaf_identity_tests.rs                     #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    #[must_use]
    pub fn is_structural_target(&self, _ty: &InferredType) -> bool {
        panic!(
            "basilisk-checker: `AnnotationResolver::is_structural_target` was DELETED \
             because it classified `Named(String)` leaves as Protocols or TypedDicts by \
             membership in a SET OF CLASS-NAME SPELLINGS. It panics because the real \
             implementation DOES NOT EXIST YET: nominal leaves must carry their resolved \
             definition site, whose class metadata supplies structural identity. Do not \
             restore the name set and do not return `true` or `false` in its place."
        )
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

    // ######################################################################
    // # DELETED BODY — `resolve_text`. DO NOT RESTORE IT.                  #
    // #                                                                     #
    // #   let parsed = ruff_python_parser::parse_expression(text.trim())?;  #
    // #   Some(self.resolve(parsed.expr()))                                 #
    // #                                                                     #
    // # Re-parsing a RENDERING is still deriving a type from characters.    #
    // # The reconstructed expression has no offset in the file, so:         #
    // #                                                                     #
    // #   * resolution cannot be positional and falls back to the module's  #
    // #     FINAL namespace — an annotation written before a rebinding      #
    // #     resolves to whatever the name means after it;                   #
    // #   * the enclosing scope is gone, so a class-local or function-local #
    // #     binding that governed the original expression is invisible;     #
    // #   * a rendering that does not round-trip resolves to something      #
    // #     else, or to nothing.                                            #
    // #                                                                     #
    // # The signature survives only as the map of callers that must be      #
    // # rebuilt to carry the annotation's own `Expr` to `resolve` /         #
    // # `resolve_span`.                                                     #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    #[must_use]
    pub fn resolve_text(&self, _text: &str) -> Option<InferredType> {
        panic!(
            "basilisk-checker: `AnnotationResolver::resolve_text` was DELETED because it \
             recovered a type by RE-PARSING a rendering, which has no position in the \
             file and therefore no scope and no positional binding. It panics because \
             the real implementation DOES NOT EXIST YET: the caller must carry the \
             annotation's `Expr` to `resolve`/`resolve_span`. Do not restore the parse-back \
             and do not return `None` in its place."
        )
    }

    // ######################################################################
    // # DELETED BODY — `is_grounded_name`. DO NOT RESTORE IT.              #
    // #                                                                     #
    // #   let parsed = ruff_python_parser::parse_expression(name.trim())?;  #
    // #   self.grounds(parsed.expr())                                       #
    // #                                                                     #
    // # Same defect as `resolve_text` above: a `&str` in, a verdict out.    #
    // # The parameter is a `&str` only because the engine's nominal leaf    #
    // # carries a RENDERING ([TYPEINF-LEGACY]). `grounds` itself is kept —  #
    // # it reads an `Expr` through the binding table and is what the        #
    // # rebuild calls once the leaf carries its definition site.            #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    #[must_use]
    pub fn is_grounded_name(&self, _name: &str) -> bool {
        panic!(
            "basilisk-checker: `AnnotationResolver::is_grounded_name` was DELETED because \
             it decided whether a leaf is grounded by RE-PARSING its rendering against the \
             module's final namespace, with no offset and no scope. It panics because the \
             real implementation DOES NOT EXIST YET: the nominal leaf must carry the \
             definition site it resolved to. Do not restore the parse-back and do not \
             return `false` in its place."
        )
    }

    /// Is `expr` a leaf the module GROUNDS — a class declared here, a member
    /// of one, or a definition the canonical registry describes?
    ///
    /// ORPHANED, NOT DELETED. This is the lawful half of the deleted
    /// `is_grounded_name`: it reads an `Expr` through the binding table and
    /// has none of that function's defects. It is exactly what the rebuild
    /// calls once the nominal leaf carries its definition site. Do not delete
    /// it to silence the dead-code lint.
    #[expect(
        dead_code,
        reason = "caller deleted for taking a rendering; this AST-based helper is the \
                  rebuild target — see the `is_grounded_name` banner"
    )]
    fn grounds(&self, expr: &Expr) -> bool {
        // `X[...]` is grounded exactly when `X` is: a subscript parameterises
        // the class its head denotes.
        let head = match expr {
            Expr::Subscript(subscript) => subscript.value.as_ref(),
            other => other,
        };
        if self.bindings.deferred_local_class(head).is_some()
            || self.bindings.deferred_form_of(head).is_some()
        {
            return true;
        }
        // `Color.RED` — a member of a class this module defines. The
        // qualifier resolves to the class; the attribute must be one it
        // declares, not merely a word after a dot.
        match head {
            Expr::Attribute(attribute) => self
                .bindings
                .deferred_local_class(&attribute.value)
                .is_some_and(|site| self.class_declares(Span::from(site), attribute.attr.as_str())),
            _ => false,
        }
    }

    /// Does the class defined at `site` declare `member` as an attribute or a
    /// method?
    ///
    /// ORPHANED with [`Self::grounds`], its only caller.
    #[expect(
        dead_code,
        reason = "reached only from `grounds`, whose caller was deleted; retained for \
                  the rebuild"
    )]
    fn class_declares(&self, site: Span, member: &str) -> bool {
        self.classes.iter().any(|class| {
            class.name_span == site
                && (class.attributes.iter().any(|attr| attr.name == member)
                    || class.method_names.iter().any(|name| name == member))
        })
    }

    /// The cascade over one type expression.
    pub(crate) fn eval(&self, expr: &Expr, frame: &Frame) -> InferredType {
        if frame.depth > MAX_DEPTH {
            return InferredType::Unknown;
        }
        match expr {
            Expr::Name(name) => match self.name(name.id.as_str(), frame) {
                // A spelling the tables cannot ground may still DENOTE a
                // builtin class through its binding (`from builtins import
                // str as Text`). The binding table answers by identity, so
                // this only ever grounds what the name provably is — it
                // never re-reads the spelling.
                InferredType::Unknown => self
                    .builtin_class_leaf(expr)
                    .unwrap_or(InferredType::Unknown),
                resolved => resolved,
            },
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
        if let Some(narrowing) = self.narrowing_form(&sub.value, &args, frame) {
            return narrowing;
        }
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

    /// The builtin class a name-or-attribute node DENOTES per the binding
    /// table, as the engine's leaf for it. `None` for anything that is not
    /// provably a builtin class — the caller keeps its own answer.
    fn builtin_class_leaf(&self, expr: &Expr) -> Option<InferredType> {
        use basilisk_resolver::TypingForm;
        Some(match self.bindings.form_of_with_builtins(expr)? {
            TypingForm::IntClass => InferredType::Int,
            TypingForm::StrClass => InferredType::Str,
            TypingForm::FloatClass => InferredType::Float,
            TypingForm::BoolClass => InferredType::Bool,
            TypingForm::BytesClass => InferredType::Bytes,
            TypingForm::NoneTypeClass => InferredType::None_,
            TypingForm::ObjectClass => InferredType::Object,
            _ => return None,
        })
    }

    /// `TypeIs[X]` (PEP 742) / `TypeGuard[X]` (PEP 647), recognised by
    /// resolving the subscript's base NODE through the binding table — so an
    /// aliased import (`from typing import TypeIs as N`), a qualified
    /// spelling (`typing_extensions.TypeIs`), and a shadowed name all lower
    /// by what they denote. Both forms take exactly one type argument; any
    /// other arity is not the narrowing form and falls through.
    fn narrowing_form(&self, base: &Expr, args: &[&Expr], frame: &Frame) -> Option<InferredType> {
        let type_is = match self.bindings.form_of(base)? {
            basilisk_resolver::TypingForm::TypeIs => true,
            basilisk_resolver::TypingForm::TypeGuard => false,
            _ => return None,
        };
        let [target] = args else { return None };
        Some(InferredType::Guard {
            type_is,
            inner: Box::new(self.eval(target, &frame.nested())),
        })
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
