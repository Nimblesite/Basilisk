//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Structural callable subtyping for callback protocols and `Callable` forms.
//!
//! E0014's name-based comparison cannot evaluate structural compatibility
//! between callback protocols (`class P(Protocol): def __call__...`),
//! `Callable[...]` annotations, `TypeAlias` callables, and member-based
//! protocols.  This module resolves annotation texts to signature sets
//! (see [`super::sig_model`]) and applies the typing spec's subtyping rules
//! (see [`super::sig_subtype`] and [`super::protocol_members`]) so the rule
//! can suppress assignments that are structurally valid.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};

use basilisk_resolver::ResolvedModule;

use crate::rules::shared::{ann_str, parse_module, split_top_level_commas};

use super::protocol_members::protocol_satisfied;
use super::sig_model::{class_entry, posonly_param, ClassEntry, Sig, StarParam, TypeSigs};
use super::sig_subtype::sig_subtype;

// ---------------------------------------------------------------------------
// Module index
// ---------------------------------------------------------------------------

/// Per-module index of classes, callable aliases, and `ParamSpec` names.
pub(super) struct CallIndex {
    /// Class name → structural entry (methods, attributes, bases).
    pub(super) classes: HashMap<String, ClassEntry>,
    /// `Name: TypeAlias = <expr>` definitions.
    aliases: HashMap<String, String>,
    /// Declared `ParamSpec` names.
    paramspecs: HashSet<String>,
}

/// Build the [`CallIndex`] for a module.
pub(super) fn build_index(module: &ResolvedModule) -> CallIndex {
    let mut index = CallIndex {
        classes: HashMap::new(),
        aliases: HashMap::new(),
        paramspecs: HashSet::new(),
    };
    let Some(parsed) = parse_module(module) else {
        return index;
    };
    for stmt in &parsed.ast.body {
        match stmt {
            Stmt::ClassDef(cls) => {
                let _ = index.classes.insert(cls.name.to_string(), class_entry(cls));
            }
            Stmt::AnnAssign(ann) => {
                let is_alias = matches!(
                    ann.annotation.as_ref(),
                    Expr::Name(n) if n.id.as_str() == "TypeAlias"
                );
                if let (true, Expr::Name(target), Some(value)) =
                    (is_alias, ann.target.as_ref(), ann.value.as_deref())
                {
                    let _ = index.aliases.insert(target.id.to_string(), ann_str(value));
                }
            }
            Stmt::Assign(assign) => {
                let is_paramspec = matches!(
                    assign.value.as_ref(),
                    Expr::Call(call) if matches!(
                        call.func.as_ref(),
                        Expr::Name(n) if n.id.as_str() == "ParamSpec"
                    )
                );
                if let (true, [Expr::Name(target)]) = (is_paramspec, assign.targets.as_slice()) {
                    let _ = index.paramspecs.insert(target.id.to_string());
                }
            }
            _ => {}
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
    if index.classes.is_empty() && index.aliases.is_empty() {
        return false;
    }

    // Member-based protocol satisfaction (both sides are indexed classes and
    // the target is a protocol) — covers non-callable protocol members too.
    let (declared_base, declared_args) = split_subscript(declared_text);
    let (rhs_base, rhs_args) = split_subscript(rhs_text);
    if let (Some(target), Some(source)) = (
        index.classes.get(declared_base),
        index.classes.get(rhs_base),
    ) {
        if target.is_protocol {
            return protocol_satisfied(
                (target, declared_args.as_deref()),
                (source, rhs_args.as_deref()),
                index,
            );
        }
    }

    // Callable-form comparison (`Callable[...]`, aliases, callback protocols).
    let Some(target) = resolve(declared_text, index, 0) else {
        return false;
    };
    let Some(source) = resolve(rhs_text, index, 0) else {
        return false;
    };
    sigs_compatible(&source, &target)
}

/// Overload-set compatibility: every target signature must be satisfied by
/// some source signature.  `Unknown` on either side is treated as compatible.
pub(super) fn sigs_compatible(source: &TypeSigs, target: &TypeSigs) -> bool {
    match (source, target) {
        (TypeSigs::Unknown, _) | (_, TypeSigs::Unknown) => true,
        (TypeSigs::Sigs(src), TypeSigs::Sigs(tgt)) => {
            !src.is_empty()
                && !tgt.is_empty()
                && tgt.iter().all(|b| src.iter().any(|a| sig_subtype(a, b)))
        }
    }
}

// ---------------------------------------------------------------------------
// Type-expression resolution
// ---------------------------------------------------------------------------

/// Resolve a type expression text into callable signatures.
/// `None` means "not a callable form we understand" — the caller keeps its flag.
fn resolve(text: &str, index: &CallIndex, depth: u8) -> Option<TypeSigs> {
    if depth > 4 {
        return None;
    }
    let text = text.trim();
    let (base, args) = split_subscript(text);

    if base == "Callable" {
        return callable_sigs(args.as_deref(), index);
    }

    if let Some(entry) = index.classes.get(base) {
        let call_sigs = entry.methods.get("__call__")?;
        return Some(specialize_class_sigs(
            call_sigs,
            &entry.generic_params,
            args.as_deref(),
            index,
        ));
    }

    if let Some(alias_rhs) = index.aliases.get(base) {
        let substituted = substitute_alias(alias_rhs, args.as_deref(), index)?;
        return resolve(&substituted, index, depth + 1);
    }

    None
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

/// Signatures for a `Callable[...]` annotation.
fn callable_sigs(args: Option<&[String]>, index: &CallIndex) -> Option<TypeSigs> {
    let args = args?;
    let [params_part, ret_part] = args else {
        return None;
    };
    let ret = Some(ret_part.clone());
    let params_part = params_part.trim();

    if params_part == "..." {
        return Some(TypeSigs::Sigs(vec![Sig {
            ret,
            gradual: true,
            ..Sig::default()
        }]));
    }
    if let Some(inner) = params_part
        .strip_prefix("Concatenate[")
        .and_then(|s| s.strip_suffix(']'))
    {
        let mut prefix: Vec<String> = split_top_level_commas(inner)
            .into_iter()
            .map(|s| s.trim().to_owned())
            .collect();
        let tail = prefix.pop()?;
        if index.paramspecs.contains(tail.as_str()) {
            return Some(TypeSigs::Unknown);
        }
        if tail != "..." {
            return None;
        }
        return Some(TypeSigs::Sigs(vec![Sig {
            positional: prefix.into_iter().map(posonly_param).collect(),
            ret,
            gradual: true,
            ..Sig::default()
        }]));
    }
    if let Some(inner) = params_part
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
    {
        let positional = split_top_level_commas(inner)
            .into_iter()
            .map(|s| posonly_param(s.trim().to_owned()))
            .collect();
        return Some(TypeSigs::Sigs(vec![Sig {
            positional,
            ret,
            ..Sig::default()
        }]));
    }
    if index.paramspecs.contains(params_part) {
        return Some(TypeSigs::Unknown);
    }
    None
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
    index: &CallIndex,
) -> TypeSigs {
    let substitutions: HashMap<&str, &str> = generic_params
        .iter()
        .zip(args.unwrap_or(&[]).iter())
        .map(|(param, arg)| (param.as_str(), arg.as_str()))
        .collect();

    let mut out = Vec::with_capacity(sigs.len());
    for sig in sigs {
        match specialize_sig(sig, &substitutions, index) {
            Some(specialized) => out.push(specialized),
            None => return TypeSigs::Unknown,
        }
    }
    TypeSigs::Sigs(out)
}

/// Apply substitutions to one signature.  Returns `None` (→ `Unknown`) for
/// `ParamSpec` signatures that aren't specialized to `...`.
fn specialize_sig(
    sig: &Sig,
    substitutions: &HashMap<&str, &str>,
    index: &CallIndex,
) -> Option<Sig> {
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

    // `*args: P.args, **kwargs: P.kwargs` — a ParamSpec signature.
    let paramspec_name = sig.vararg.ty().and_then(|ann| {
        let name = ann.strip_suffix(".args")?;
        index.paramspecs.contains(name).then(|| name.to_owned())
    });
    if let Some(ps) = paramspec_name {
        let specialization = substitutions.get(ps.as_str()).copied();
        return match specialization {
            // `Proto[...]` — gradual with the non-ParamSpec params as prefix.
            Some("...") => Some(Sig {
                positional: sig.positional.clone(),
                kwonly: sig.kwonly.clone(),
                vararg: StarParam::Absent,
                kwarg: StarParam::Absent,
                ret: sig.ret.clone(),
                gradual: true,
            }),
            // Bare or absent ParamSpec — not evaluable.
            _ => None,
        };
    }

    let map_params = |params: &[super::sig_model::Param]| {
        params
            .iter()
            .map(|p| super::sig_model::Param {
                ty: subst(&p.ty),
                ..p.clone()
            })
            .collect()
    };
    Some(Sig {
        positional: map_params(&sig.positional),
        kwonly: map_params(&sig.kwonly),
        vararg: subst_star(&sig.vararg),
        kwarg: subst_star(&sig.kwarg),
        ret: subst(&sig.ret),
        gradual: sig.gradual,
    })
}

/// Substitute a single-free-parameter alias's parameter with the use-site
/// argument (e.g. `Callback[...]` with `Callback = Callable[P, str]`).
fn substitute_alias(alias_rhs: &str, args: Option<&[String]>, index: &CallIndex) -> Option<String> {
    let Some([arg]) = args else {
        // Bare alias use — keep the RHS as-is.
        return args.is_none().then(|| alias_rhs.to_owned());
    };
    let free: Vec<&str> = index
        .paramspecs
        .iter()
        .map(String::as_str)
        .filter(|ps| contains_word(alias_rhs, ps))
        .collect();
    let [single] = free.as_slice() else {
        return None;
    };
    Some(replace_word(alias_rhs, single, arg))
}

/// `true` when `word` appears as a whole identifier in `text`.
pub(super) fn contains_word(text: &str, word: &str) -> bool {
    replace_word(text, word, "\u{0}") != text
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
