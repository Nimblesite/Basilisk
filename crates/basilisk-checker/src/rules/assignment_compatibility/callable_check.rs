//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Structural callable subtyping for classes with `__call__` methods.
//!
//! E0014's name-based comparison cannot evaluate structural compatibility
//! between classes defining `__call__`.  This module resolves annotation
//! texts to signature sets (see [`super::sig_model`]) and applies the typing
//! spec's subtyping rules (see [`super::sig_subtype`]) so the rule can
//! suppress assignments that are structurally valid.

use std::collections::HashMap;

use ruff_python_ast::Stmt;

use basilisk_resolver::ResolvedModule;

use crate::rules::shared::{parse_module, split_top_level_commas};

use super::sig_model::{class_entry, ClassEntry, Sig, StarParam, TypeSigs};
use super::sig_subtype::sig_subtype;

// ---------------------------------------------------------------------------
// Module index
// ---------------------------------------------------------------------------

/// Per-module index of classes.
pub(super) struct CallIndex {
    /// Class name → structural entry (methods, attributes, bases).
    pub(super) classes: HashMap<String, ClassEntry>,
    /// Module-seeded nominal context — the ONE subtyping implementation
    /// every signature verdict routes through ([NARROWPLAN-SUBTYPING]).
    pub(super) subtyping: crate::subtyping::SubtypingContext,
}

/// Build the [`CallIndex`] for a module.
pub(super) fn build_index(module: &ResolvedModule) -> CallIndex {
    let mut index = CallIndex {
        classes: HashMap::new(),
        subtyping: crate::subtyping::module_context(module),
    };
    let Some(parsed) = parse_module(module) else {
        return index;
    };
    for stmt in &parsed.ast.body {
        if let Stmt::ClassDef(cls) = stmt {
            let _ = index.classes.insert(cls.name.to_string(), class_entry(cls));
        }
    }
    index
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// `true` when an assignment of a value of type `rhs_text` to a variable
/// annotated `declared_text` is structurally valid subtyping.
/// `false` means "not provably valid" — the caller keeps its diagnostic.
pub(super) fn assignment_compatible(
    declared_text: &str,
    rhs_text: &str,
    index: &CallIndex,
) -> bool {
    if index.classes.is_empty() {
        return false;
    }
    let Some(target) = resolve(declared_text, index) else {
        return false;
    };
    let Some(source) = resolve(rhs_text, index) else {
        return false;
    };
    sigs_compatible(&index.subtyping, &source, &target)
}

/// Overload-set compatibility: every target signature must be satisfied by
/// some source signature.  `Unknown` on either side is treated as compatible.
pub(super) fn sigs_compatible(
    subtyping: &crate::subtyping::SubtypingContext,
    source: &TypeSigs,
    target: &TypeSigs,
) -> bool {
    match (source, target) {
        (TypeSigs::Unknown, _) | (_, TypeSigs::Unknown) => true,
        (TypeSigs::Sigs(src), TypeSigs::Sigs(tgt)) => {
            !src.is_empty()
                && !tgt.is_empty()
                && tgt
                    .iter()
                    .all(|b| src.iter().any(|a| sig_subtype(subtyping, a, b)))
        }
    }
}

// ---------------------------------------------------------------------------
// Type-expression resolution
// ---------------------------------------------------------------------------

/// Resolve a type expression text into callable signatures.
/// `None` means "not a callable form we understand" — the caller keeps its flag.
fn resolve(text: &str, index: &CallIndex) -> Option<TypeSigs> {
    let text = text.trim();
    let (base, args) = split_subscript(text);
    let entry = index.classes.get(base)?;
    let call_sigs = entry.methods.get("__call__")?;
    Some(specialize_class_sigs(
        call_sigs,
        &entry.generic_params,
        args.as_deref(),
        index,
    ))
}

/// Split `Name[args]` into `("Name", Some(["arg", ...]))`; bare names get `None`.
fn split_subscript(text: &str) -> (&str, Option<Vec<String>>) {
    let text = text.trim();
    let Some(bracket) = text.find('[') else {
        return (text, None);
    };
    let base = text[..bracket].trim();
    let inner = text[bracket + 1..]
        .strip_suffix(']')
        .unwrap_or(&text[bracket + 1..]);
    let args = split_top_level_commas(inner)
        .into_iter()
        .map(|s| s.trim().to_owned())
        .collect();
    (base, Some(args))
}

// ---------------------------------------------------------------------------
// Specialization
// ---------------------------------------------------------------------------

/// Specialize a class's method signatures with subscript arguments
/// (e.g. `Proto5[Any]` substitutes `T_contra := Any`).
pub(super) fn specialize_class_sigs(
    sigs: &[Sig],
    generic_params: &[String],
    args: Option<&[String]>,
    _index: &CallIndex,
) -> TypeSigs {
    let substitutions: HashMap<&str, &str> = generic_params
        .iter()
        .zip(args.unwrap_or(&[]).iter())
        .map(|(param, arg)| (param.as_str(), arg.as_str()))
        .collect();

    TypeSigs::Sigs(
        sigs.iter()
            .map(|sig| specialize_sig(sig, &substitutions))
            .collect(),
    )
}

/// Apply substitutions to one signature.
fn specialize_sig(sig: &Sig, substitutions: &HashMap<&str, &str>) -> Sig {
    let subst_text = |t: &str| -> String {
        substitutions
            .iter()
            .fold(t.to_owned(), |text, (param, arg)| {
                replace_word(&text, param, arg)
            })
    };
    let subst = |ty: &Option<String>| -> Option<String> { ty.as_deref().map(subst_text) };
    let subst_star = |star: &StarParam| -> StarParam {
        match star {
            StarParam::Typed(ty) => StarParam::Typed(subst_text(ty)),
            other => other.clone(),
        }
    };

    let map_params = |params: &[super::sig_model::Param]| {
        params
            .iter()
            .map(|p| super::sig_model::Param {
                ty: subst(&p.ty),
                ..p.clone()
            })
            .collect()
    };
    Sig {
        positional: map_params(&sig.positional),
        kwonly: map_params(&sig.kwonly),
        vararg: subst_star(&sig.vararg),
        kwarg: subst_star(&sig.kwarg),
        ret: subst(&sig.ret),
        gradual: sig.gradual,
    }
}

/// Replace whole-identifier occurrences of `word` in `text`.
pub(super) fn replace_word(text: &str, word: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    while let Some(pos) = rest.find(word) {
        let before_ok = !rest[..pos].chars().next_back().is_some_and(is_ident);
        let after = &rest[pos + word.len()..];
        let after_ok = !after.chars().next().is_some_and(is_ident);
        out.push_str(&rest[..pos]);
        if before_ok && after_ok {
            out.push_str(replacement);
        } else {
            out.push_str(word);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}
