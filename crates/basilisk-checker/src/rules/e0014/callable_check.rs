//! Implements [BSK-E0014] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Structural callable subtyping for callback protocols and `Callable` forms.
//!
//! E0014's name-based comparison cannot evaluate structural compatibility
//! between callback protocols (`class P(Protocol): def __call__...`),
//! `Callable[...]` annotations, and `TypeAlias` callables.  This module
//! implements the typing spec's "subtyping rules for callables" so the rule
//! can suppress assignments that are structurally valid:
//!
//! - positional-only / standard / keyword-only parameter matching
//! - `*args` / `**kwargs` contravariance
//! - default-argument elision
//! - overloads (source: any overload matches; target: all overloads match)
//! - gradual forms: `Callable[..., R]`, `Concatenate[T, ...]`, and
//!   `*args: Any, **kwargs: Any` signatures (treated as `...` per the spec)
//! - `ParamSpec` signatures (not evaluable — treated as compatible)

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};

use basilisk_resolver::ResolvedModule;

use crate::rules::shared::{ann_str, is_numeric_subtype, parse_module, split_top_level_commas};

// ---------------------------------------------------------------------------
// Signature model
// ---------------------------------------------------------------------------

/// One callable parameter.
#[derive(Debug, Clone)]
struct Param {
    name: String,
    ty: Option<String>,
    has_default: bool,
    /// `true` for positional-or-keyword ("standard") parameters.
    is_standard: bool,
}

/// A parsed callable signature.
#[derive(Debug, Clone, Default)]
struct Sig {
    /// Positional parameters (positional-only first, then standard).
    positional: Vec<Param>,
    kwonly: Vec<Param>,
    /// `Some(ty)` when `*args` is present (`None` type = unannotated/Any).
    vararg: Option<Option<String>>,
    /// `Some(ty)` when `**kwargs` is present.
    kwarg: Option<Option<String>>,
    ret: Option<String>,
    /// `true` when the parameter list is gradual (`...`): `positional` then
    /// holds the required `Concatenate` prefix and `kwonly` any retained
    /// keyword-only parameters.
    gradual: bool,
}

/// The resolved signatures of a type expression.
enum TypeSigs {
    /// Involves a `ParamSpec` or another non-evaluable form — treat as compatible.
    Unknown,
    /// Concrete overload set.
    Sigs(Vec<Sig>),
}

// ---------------------------------------------------------------------------
// Module index
// ---------------------------------------------------------------------------

/// Per-module index of callback-protocol classes and callable aliases.
pub(super) struct CallIndex {
    /// Class name → (`__call__` overload signatures, generic parameter names).
    classes: HashMap<String, (Vec<Sig>, Vec<String>)>,
    /// `Name: TypeAlias = <expr>` definitions.
    aliases: HashMap<String, String>,
    /// Declared `ParamSpec` names.
    paramspecs: HashSet<String>,
}

/// Build the [`CallIndex`] for a module.  Returns an empty index when the
/// module has no callable classes or aliases.
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
                let sigs = call_method_sigs(cls);
                if !sigs.is_empty() {
                    let _ = index
                        .classes
                        .insert(cls.name.to_string(), (sigs, generic_param_names(cls)));
                }
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

/// Extract `__call__` overload signatures from a class body.
fn call_method_sigs(cls: &ruff_python_ast::StmtClassDef) -> Vec<Sig> {
    let defs: Vec<&ruff_python_ast::StmtFunctionDef> = cls
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::FunctionDef(f) if f.name.as_str() == "__call__" => Some(f),
            _ => None,
        })
        .collect();
    let has_overloads = defs.iter().any(|f| {
        f.decorator_list
            .iter()
            .any(|d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload"))
    });
    defs.iter()
        .filter(|f| {
            !has_overloads
                || f.decorator_list
                    .iter()
                    .any(|d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload"))
        })
        .map(|f| sig_from_function(f))
        .collect()
}

/// Generic parameter names of a class: PEP 695 params plus `Protocol[...]` /
/// `Generic[...]` subscript arguments.
fn generic_param_names(cls: &ruff_python_ast::StmtClassDef) -> Vec<String> {
    let mut names: Vec<String> = cls
        .type_params
        .as_ref()
        .map(|tp| tp.type_params.iter().map(|p| p.name().to_string()).collect())
        .unwrap_or_default();
    for base in cls.bases() {
        let Expr::Subscript(sub) = base else { continue };
        let base_name = ann_str(&sub.value);
        if base_name != "Protocol" && base_name != "Generic" {
            continue;
        }
        let args: Vec<&Expr> = match sub.slice.as_ref() {
            Expr::Tuple(t) => t.elts.iter().collect(),
            other => vec![other],
        };
        names.extend(args.iter().filter_map(|a| match a {
            Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        }));
    }
    names
}

/// Build a [`Sig`] from a `__call__` definition (drops `self`).
fn sig_from_function(func: &ruff_python_ast::StmtFunctionDef) -> Sig {
    let params = &func.parameters;
    let to_param = |pwd: &ruff_python_ast::ParameterWithDefault, is_standard: bool| Param {
        name: pwd.parameter.name.to_string(),
        ty: pwd.parameter.annotation.as_deref().map(ann_str),
        has_default: pwd.default.is_some(),
        is_standard,
    };
    let mut positional: Vec<Param> = params
        .posonlyargs
        .iter()
        .map(|p| to_param(p, false))
        .chain(params.args.iter().map(|p| to_param(p, true)))
        .collect();
    if !positional.is_empty() {
        let _ = positional.remove(0); // self
    }
    let kwonly = params.kwonlyargs.iter().map(|p| to_param(p, false)).collect();
    let vararg = params
        .vararg
        .as_ref()
        .map(|v| v.annotation.as_deref().map(ann_str));
    let kwarg = params
        .kwarg
        .as_ref()
        .map(|k| k.annotation.as_deref().map(ann_str));
    let ret = func.returns.as_deref().map(ann_str);

    let mut sig = Sig {
        positional,
        kwonly,
        vararg,
        kwarg,
        ret,
        gradual: false,
    };
    // `*args: Any, **kwargs: Any` (literally annotated or unannotated) is
    // equivalent to `...` per the typing spec; other parameters are retained.
    let is_any = |ann: &Option<String>| ann.as_deref().is_none_or(|t| t == "Any");
    if let (Some(va), Some(ka)) = (&sig.vararg, &sig.kwarg) {
        if is_any(va) && is_any(ka) {
            sig.gradual = true;
            sig.vararg = None;
            sig.kwarg = None;
        }
    }
    sig
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

    if let Some((sigs, generic_params)) = index.classes.get(base) {
        return Some(specialize_class_sigs(sigs, generic_params, args.as_deref(), index));
    }

    if let Some(alias_rhs) = index.aliases.get(base) {
        let substituted = substitute_alias(alias_rhs, args.as_deref(), index)?;
        return resolve(&substituted, index, depth + 1);
    }

    None
}

/// Split `Name[args]` into `("Name", Some(["arg", ...]))`; bare names get `None`.
fn split_subscript(text: &str) -> (&str, Option<Vec<String>>) {
    let Some(bracket) = text.find('[') else {
        return (text, None);
    };
    let base = text[..bracket].trim();
    let inner = text[bracket + 1..].strip_suffix(']').unwrap_or(&text[bracket + 1..]);
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
    if let Some(inner) = params_part.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
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

/// An anonymous positional-only parameter of type `ty`.
fn posonly_param(ty: String) -> Param {
    Param {
        name: String::new(),
        ty: Some(ty),
        has_default: false,
        is_standard: false,
    }
}

/// Specialize a protocol's `__call__` signatures with subscript arguments
/// (e.g. `Proto5[Any]` substitutes `T_contra := Any`).
fn specialize_class_sigs(
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
    let subst = |ty: &Option<String>| -> Option<String> {
        ty.as_deref()
            .map(|t| substitutions.get(t).map_or_else(|| t.to_owned(), |s| (*s).to_owned()))
    };

    // `*args: P.args, **kwargs: P.kwargs` — a ParamSpec signature.
    let paramspec_name = sig.vararg.as_ref().and_then(|va| {
        let ann = va.as_deref()?;
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
                vararg: None,
                kwarg: None,
                ret: sig.ret.clone(),
                gradual: true,
            }),
            // Bare or absent ParamSpec — not evaluable.
            _ => None,
        };
    }

    let map_params = |params: &[Param]| {
        params
            .iter()
            .map(|p| Param {
                ty: subst(&p.ty),
                ..p.clone()
            })
            .collect()
    };
    Some(Sig {
        positional: map_params(&sig.positional),
        kwonly: map_params(&sig.kwonly),
        vararg: sig.vararg.as_ref().map(|inner| subst(inner)),
        kwarg: sig.kwarg.as_ref().map(|inner| subst(inner)),
        ret: subst(&sig.ret),
        gradual: sig.gradual,
    })
}

/// Substitute a single-free-parameter alias's parameter with the use-site
/// argument (e.g. `Callback[...]` with `Callback = Callable[P, str]`).
fn substitute_alias(
    alias_rhs: &str,
    args: Option<&[String]>,
    index: &CallIndex,
) -> Option<String> {
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

// ---------------------------------------------------------------------------
// Subtyping
// ---------------------------------------------------------------------------

/// `true` when an assignment of a value of type `rhs_text` to a variable
/// annotated `declared_text` is structurally valid callable subtyping.
/// `false` means "not provably valid" — the caller keeps its diagnostic.
pub(super) fn assignment_compatible(
    declared_text: &str,
    rhs_text: &str,
    index: &CallIndex,
) -> bool {
    if index.classes.is_empty() && index.aliases.is_empty() {
        return false;
    }
    let Some(target) = resolve(declared_text, index, 0) else {
        return false;
    };
    let Some(source) = resolve(rhs_text, index, 0) else {
        return false;
    };
    match (source, target) {
        (TypeSigs::Unknown, _) | (_, TypeSigs::Unknown) => true,
        (TypeSigs::Sigs(src), TypeSigs::Sigs(tgt)) => {
            !src.is_empty()
                && !tgt.is_empty()
                && tgt
                    .iter()
                    .all(|b| src.iter().any(|a| sig_subtype(a, b)))
        }
    }
}

/// `true` when signature `a` (source) is a subtype of `b` (target).
fn sig_subtype(a: &Sig, b: &Sig) -> bool {
    if !ty_subtype(a.ret.as_deref(), b.ret.as_deref()) {
        return false;
    }
    if b.gradual {
        return gradual_target_ok(a, b);
    }
    if a.gradual {
        return gradual_source_ok(a, b);
    }
    concrete_subtype(a, b)
}

/// Target is gradual (`...` with optional prefix): check the prefix and any
/// retained keyword-only parameters; everything else is unchecked.
fn gradual_target_ok(a: &Sig, b: &Sig) -> bool {
    for (idx, bp) in b.positional.iter().enumerate() {
        let accepted = a.positional.get(idx).map_or_else(
            || a.gradual || a.vararg.is_some(),
            |ap| ty_subtype(bp.ty.as_deref(), ap.ty.as_deref()),
        );
        if !accepted {
            return false;
        }
    }
    b.kwonly.iter().all(|bk| keyword_accepted(a, bk))
}

/// Source is gradual: its prefix parameters are real requirements that the
/// target's positional arguments must satisfy.
fn gradual_source_ok(a: &Sig, b: &Sig) -> bool {
    for (idx, ap) in a.positional.iter().enumerate() {
        let supplied = b
            .positional
            .get(idx)
            .map(|bp| bp.ty.as_deref().map(ToOwned::to_owned))
            .or_else(|| b.vararg.clone());
        let Some(supplied_ty) = supplied else {
            if ap.has_default {
                continue;
            }
            return false;
        };
        if !ty_subtype(supplied_ty.as_deref(), ap.ty.as_deref()) {
            return false;
        }
    }
    a.kwonly
        .iter()
        .filter(|ak| !ak.has_default)
        .all(|ak| keyword_supplied(b, ak))
}

/// Full concrete-vs-concrete subtyping per the typing spec.
fn concrete_subtype(a: &Sig, b: &Sig) -> bool {
    let mut a_idx = 0usize;
    let mut consumed: HashSet<&str> = HashSet::new();

    for bp in &b.positional {
        if let Some(ap) = a.positional.get(a_idx) {
            if !ty_subtype(bp.ty.as_deref(), ap.ty.as_deref()) {
                return false;
            }
            if bp.is_standard && (!ap.is_standard || ap.name != bp.name) {
                return false;
            }
            if bp.has_default && !ap.has_default {
                return false;
            }
            let _ = consumed.insert(ap.name.as_str());
            a_idx += 1;
        } else if let Some(va) = &a.vararg {
            if !ty_subtype(bp.ty.as_deref(), va.as_deref()) {
                return false;
            }
            if bp.is_standard && !keyword_accepted(a, bp) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Match target keyword-only params first — they may consume leftover
    // source standard params by name (`KwOnly = standard` is valid).
    for bk in &b.kwonly {
        if !keyword_matched(a, bk, &mut consumed) {
            return false;
        }
    }

    if !vararg_compatible(a, b, a_idx, &consumed) {
        return false;
    }
    if b.vararg.is_none() {
        // Leftover source positionals must be optional or keyword-consumed.
        let unmet = a.positional[a_idx..]
            .iter()
            .any(|ap| !ap.has_default && !consumed.contains(ap.name.as_str()));
        if unmet {
            return false;
        }
    }

    kwarg_compatible(a, b, &consumed)
}

/// `*args` compatibility: a target `*args` requires a source `*args` with a
/// supertype element, and any extra source positionals must absorb it.
fn vararg_compatible(a: &Sig, b: &Sig, a_idx: usize, consumed: &HashSet<&str>) -> bool {
    let Some(bv) = &b.vararg else {
        return true;
    };
    for ap in &a.positional[a_idx..] {
        if consumed.contains(ap.name.as_str()) {
            continue;
        }
        if !ap.has_default || !ty_subtype(bv.as_deref(), ap.ty.as_deref()) {
            return false;
        }
    }
    a.vararg
        .as_ref()
        .is_some_and(|av| ty_subtype(bv.as_deref(), av.as_deref()))
}

/// `**kwargs` compatibility, including unmatched source keyword-only params.
fn kwarg_compatible(a: &Sig, b: &Sig, consumed: &HashSet<&str>) -> bool {
    let unconsumed = a.kwonly.iter().filter(|ak| !consumed.contains(ak.name.as_str()));
    if let Some(bkw) = &b.kwarg {
        let Some(akw) = &a.kwarg else {
            return false;
        };
        if !ty_subtype(bkw.as_deref(), akw.as_deref()) {
            return false;
        }
        for ak in unconsumed {
            if !ak.has_default || !ty_subtype(bkw.as_deref(), ak.ty.as_deref()) {
                return false;
            }
        }
        true
    } else {
        unconsumed.into_iter().all(|ak| ak.has_default)
    }
}

/// Match one target keyword-only parameter against the source, consuming the
/// matched source parameter.
fn keyword_matched<'a>(a: &'a Sig, bk: &Param, consumed: &mut HashSet<&'a str>) -> bool {
    let named = a
        .kwonly
        .iter()
        .chain(a.positional.iter().filter(|p| p.is_standard))
        .find(|ap| ap.name == bk.name && !consumed.contains(ap.name.as_str()));
    if let Some(ap) = named {
        if !ty_subtype(bk.ty.as_deref(), ap.ty.as_deref()) {
            return false;
        }
        if bk.has_default && !ap.has_default {
            return false;
        }
        let _ = consumed.insert(ap.name.as_str());
        return true;
    }
    a.kwarg
        .as_ref()
        .is_some_and(|akw| ty_subtype(bk.ty.as_deref(), akw.as_deref()))
}

/// `true` when the source can accept keyword `bk` (by name or `**kwargs`).
fn keyword_accepted(a: &Sig, bk: &Param) -> bool {
    if a.gradual {
        return true;
    }
    let named = a
        .kwonly
        .iter()
        .chain(a.positional.iter().filter(|p| p.is_standard))
        .find(|ap| ap.name == bk.name);
    match named {
        Some(ap) => ty_subtype(bk.ty.as_deref(), ap.ty.as_deref()),
        None => a
            .kwarg
            .as_ref()
            .is_some_and(|akw| ty_subtype(bk.ty.as_deref(), akw.as_deref())),
    }
}

/// `true` when the target supplies required source keyword `ak`.
fn keyword_supplied(b: &Sig, ak: &Param) -> bool {
    let named = b
        .kwonly
        .iter()
        .chain(b.positional.iter().filter(|p| p.is_standard))
        .find(|bp| bp.name == ak.name);
    match named {
        Some(bp) => ty_subtype(bp.ty.as_deref(), ak.ty.as_deref()),
        None => b
            .kwarg
            .as_ref()
            .is_some_and(|bkw| ty_subtype(bkw.as_deref(), ak.ty.as_deref())),
    }
}

// ---------------------------------------------------------------------------
// Type-text subtyping
// ---------------------------------------------------------------------------

/// `true` when type text `sub` is a subtype of `sup`.  Unannotated types are
/// treated as `Any` (compatible in both directions).
fn ty_subtype(sub: Option<&str>, sup: Option<&str>) -> bool {
    let (Some(sub), Some(sup)) = (sub, sup) else {
        return true;
    };
    let sub = sub.trim();
    let sup = sup.trim();
    if sub == sup || sub == "Any" || sup == "Any" || sup == "object" {
        return true;
    }
    let sub_parts = split_union(sub);
    if sub_parts.len() > 1 {
        return sub_parts.iter().all(|part| ty_subtype(Some(part), Some(sup)));
    }
    let sup_parts = split_union(sup);
    if sup_parts.len() > 1 {
        return sup_parts.iter().any(|part| ty_subtype(Some(sub), Some(part)));
    }
    is_numeric_subtype(sub, sup)
}

/// Split a type text at top-level `|`.
fn split_union(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
}

/// `true` when `word` appears as a whole identifier in `text`.
fn contains_word(text: &str, word: &str) -> bool {
    replace_word(text, word, "\u{0}") != text
}

/// Replace whole-identifier occurrences of `word` in `text`.
fn replace_word(text: &str, word: &str, replacement: &str) -> String {
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
