//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! The callable-subtyping algorithm from the typing spec: positional/keyword
//! matching, `*args`/`**kwargs` contravariance, defaults, and gradual forms.
//! See <https://typing.python.org/en/latest/spec/callables.html#assignability-rules-for-callables>.
//!
//! Every answer is three-valued ([ASTREBUILD-LAW]): `Some(true)` and
//! `Some(false)` are verdicts the resolved structure licenses — parameter
//! kinds, names, and defaults come from the AST, and type relations from the
//! canonical [`assignable`] relation ([RESOLV-CANONICAL-RELATION]).  `None`
//! is honest abstention (e.g. a parameter type the relation layer does not
//! model), on which no caller may base a diagnostic.

use std::collections::HashSet;

use basilisk_resolver::{assignable, TypeNode};

use super::sig_model::{Param, Sig};

/// Whether signature `a` (source) is a subtype of `b` (target).
pub(super) fn sig_subtype(a: &Sig, b: &Sig) -> Option<bool> {
    let mut verdict = Some(true);
    if !and_step(&mut verdict, ty_subtype(a.ret.as_ref(), b.ret.as_ref())) {
        return verdict;
    }
    let params = if b.gradual {
        gradual_target_ok(a, b)
    } else if a.gradual {
        gradual_source_ok(a, b)
    } else {
        concrete_subtype(a, b)
    };
    let _ = and_step(&mut verdict, params);
    verdict
}

/// Fold one three-valued answer into a running conjunction.  Returns `false`
/// when the conjunction is decided (`Some(false)`) and iteration may stop;
/// an unknown answer taints the verdict to `None` but continues, so a later
/// definite mismatch can still decide.
fn and_step(verdict: &mut Option<bool>, answer: Option<bool>) -> bool {
    match answer {
        Some(false) => {
            *verdict = Some(false);
            false
        }
        Some(true) => true,
        None => {
            *verdict = None;
            true
        }
    }
}

/// Target is gradual (`...` with optional prefix): check the prefix and any
/// retained keyword-only parameters; everything else is unchecked.
fn gradual_target_ok(a: &Sig, b: &Sig) -> Option<bool> {
    let mut verdict = Some(true);
    for (idx, bp) in b.positional.iter().enumerate() {
        let accepted = a.positional.get(idx).map_or_else(
            || Some(a.gradual || a.vararg.is_present()),
            |ap| ty_subtype(bp.ty.as_ref(), ap.ty.as_ref()),
        );
        if !and_step(&mut verdict, accepted) {
            return verdict;
        }
    }
    for bk in &b.kwonly {
        if !and_step(&mut verdict, keyword_accepted(a, bk)) {
            return verdict;
        }
    }
    verdict
}

/// Source is gradual: its prefix parameters are real requirements that the
/// target's positional arguments must satisfy.
fn gradual_source_ok(a: &Sig, b: &Sig) -> Option<bool> {
    let mut verdict = Some(true);
    for (idx, ap) in a.positional.iter().enumerate() {
        let supplied: Option<Option<&TypeNode>> = b
            .positional
            .get(idx)
            .map(|bp| bp.ty.as_ref())
            .or_else(|| b.vararg.is_present().then(|| b.vararg.ty()));
        let Some(supplied_ty) = supplied else {
            if ap.has_default {
                continue;
            }
            return Some(false);
        };
        if !and_step(&mut verdict, ty_subtype(supplied_ty, ap.ty.as_ref())) {
            return verdict;
        }
    }
    for ak in a.kwonly.iter().filter(|ak| !ak.has_default) {
        if !and_step(&mut verdict, keyword_supplied(b, ak)) {
            return verdict;
        }
    }
    verdict
}

/// Full concrete-vs-concrete subtyping per the typing spec.
fn concrete_subtype(a: &Sig, b: &Sig) -> Option<bool> {
    let mut verdict = Some(true);
    let mut a_idx = 0usize;
    let mut consumed: HashSet<&str> = HashSet::new();

    for bp in &b.positional {
        if let Some(ap) = a.positional.get(a_idx) {
            if !and_step(&mut verdict, ty_subtype(bp.ty.as_ref(), ap.ty.as_ref())) {
                return verdict;
            }
            if bp.is_standard && (!ap.is_standard || ap.name != bp.name) {
                return Some(false);
            }
            if bp.has_default && !ap.has_default {
                return Some(false);
            }
            let _ = consumed.insert(ap.name.as_str());
            a_idx += 1;
        } else if a.vararg.is_present() {
            if !and_step(&mut verdict, ty_subtype(bp.ty.as_ref(), a.vararg.ty())) {
                return verdict;
            }
            if bp.is_standard && !and_step(&mut verdict, keyword_accepted(a, bp)) {
                return verdict;
            }
        } else {
            return Some(false);
        }
    }

    // Match target keyword-only params first — they may consume leftover
    // source standard params by name (`KwOnly = standard` is valid).
    for bk in &b.kwonly {
        if !and_step(&mut verdict, keyword_matched(a, bk, &mut consumed)) {
            return verdict;
        }
    }

    if !and_step(&mut verdict, vararg_compatible(a, b, a_idx, &consumed)) {
        return verdict;
    }
    if !b.vararg.is_present() {
        // Leftover source positionals must be optional or keyword-consumed.
        let unmet = a
            .positional
            .get(a_idx..)
            .unwrap_or(&[])
            .iter()
            .any(|ap| !ap.has_default && !consumed.contains(ap.name.as_str()));
        if unmet {
            return Some(false);
        }
    }

    let _ = and_step(&mut verdict, kwarg_compatible(a, b, &consumed));
    verdict
}

/// `*args` compatibility: a target `*args` requires a source `*args` with a
/// supertype element, and any extra source positionals must absorb it.
fn vararg_compatible(a: &Sig, b: &Sig, a_idx: usize, consumed: &HashSet<&str>) -> Option<bool> {
    if !b.vararg.is_present() {
        return Some(true);
    }
    let bv = b.vararg.ty();
    let mut verdict = Some(true);
    for ap in a.positional.get(a_idx..).unwrap_or(&[]) {
        if consumed.contains(ap.name.as_str()) {
            continue;
        }
        if !ap.has_default {
            return Some(false);
        }
        if !and_step(&mut verdict, ty_subtype(bv, ap.ty.as_ref())) {
            return verdict;
        }
    }
    if !a.vararg.is_present() {
        return Some(false);
    }
    let _ = and_step(&mut verdict, ty_subtype(bv, a.vararg.ty()));
    verdict
}

/// `**kwargs` compatibility, including unmatched source keyword-only params.
fn kwarg_compatible(a: &Sig, b: &Sig, consumed: &HashSet<&str>) -> Option<bool> {
    let unconsumed = a
        .kwonly
        .iter()
        .filter(|ak| !consumed.contains(ak.name.as_str()));
    if b.kwarg.is_present() {
        if !a.kwarg.is_present() {
            return Some(false);
        }
        let bkw = b.kwarg.ty();
        let mut verdict = Some(true);
        if !and_step(&mut verdict, ty_subtype(bkw, a.kwarg.ty())) {
            return verdict;
        }
        for ak in unconsumed {
            if !ak.has_default {
                return Some(false);
            }
            if !and_step(&mut verdict, ty_subtype(bkw, ak.ty.as_ref())) {
                return verdict;
            }
        }
        verdict
    } else {
        Some(unconsumed.into_iter().all(|ak| ak.has_default))
    }
}

/// Match one target keyword-only parameter against the source, consuming the
/// matched source parameter.
fn keyword_matched<'a>(a: &'a Sig, bk: &Param, consumed: &mut HashSet<&'a str>) -> Option<bool> {
    let named = a
        .kwonly
        .iter()
        .chain(a.positional.iter().filter(|p| p.is_standard))
        .find(|ap| ap.name == bk.name && !consumed.contains(ap.name.as_str()));
    if let Some(ap) = named {
        if bk.has_default && !ap.has_default {
            return Some(false);
        }
        let _ = consumed.insert(ap.name.as_str());
        return ty_subtype(bk.ty.as_ref(), ap.ty.as_ref());
    }
    if a.kwarg.is_present() {
        return ty_subtype(bk.ty.as_ref(), a.kwarg.ty());
    }
    Some(false)
}

/// Whether the source can accept keyword `bk` (by name or `**kwargs`).
fn keyword_accepted(a: &Sig, bk: &Param) -> Option<bool> {
    if a.gradual {
        return Some(true);
    }
    let named = a
        .kwonly
        .iter()
        .chain(a.positional.iter().filter(|p| p.is_standard))
        .find(|ap| ap.name == bk.name);
    match named {
        Some(ap) => ty_subtype(bk.ty.as_ref(), ap.ty.as_ref()),
        None if a.kwarg.is_present() => ty_subtype(bk.ty.as_ref(), a.kwarg.ty()),
        None => Some(false),
    }
}

/// Whether the target supplies required source keyword `ak`.
fn keyword_supplied(b: &Sig, ak: &Param) -> Option<bool> {
    let named = b
        .kwonly
        .iter()
        .chain(b.positional.iter().filter(|p| p.is_standard))
        .find(|bp| bp.name == ak.name);
    match named {
        Some(bp) => ty_subtype(bp.ty.as_ref(), ak.ty.as_ref()),
        None if b.kwarg.is_present() => ty_subtype(b.kwarg.ty(), ak.ty.as_ref()),
        None => Some(false),
    }
}

/// Relate two lowered parameter/return types through the canonical
/// [`assignable`] relation ([RESOLV-CANONICAL-RELATION]).  `None` on either
/// SIDE means the parameter is unannotated — gradual, compatible in both
/// directions.  `None` as a RESULT is honest abstention: the relation cannot
/// prove either verdict (e.g. a user class, which [`TypeNode`] lowers to
/// `Unknown`), and no diagnostic may rest on it ([ASTREBUILD-LAW],
/// [ASTREBUILD-PHASE-RESOLVER]).
fn ty_subtype(narrow: Option<&TypeNode>, wide: Option<&TypeNode>) -> Option<bool> {
    let (Some(narrow), Some(wide)) = (narrow, wide) else {
        return Some(true);
    };
    assignable(narrow, wide)
}
