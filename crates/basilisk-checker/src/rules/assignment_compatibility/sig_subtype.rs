//! Implements [assignment_compatibility] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! The callable-subtyping algorithm from the typing spec: positional/keyword
//! matching, `*args`/`**kwargs` contravariance, defaults, and gradual forms.

use std::collections::HashSet;

use crate::rules::shared::{is_numeric_subtype, split_top_level_commas};

use super::sig_model::{Param, Sig};

/// `true` when signature `a` (source) is a subtype of `b` (target).
pub(super) fn sig_subtype(a: &Sig, b: &Sig) -> bool {
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
            || a.gradual || a.vararg.is_present(),
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
        let supplied: Option<Option<&str>> = b
            .positional
            .get(idx)
            .map(|bp| bp.ty.as_deref())
            .or_else(|| b.vararg.is_present().then(|| b.vararg.ty()));
        let Some(supplied_ty) = supplied else {
            if ap.has_default {
                continue;
            }
            return false;
        };
        if !ty_subtype(supplied_ty, ap.ty.as_deref()) {
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
        } else if a.vararg.is_present() {
            if !ty_subtype(bp.ty.as_deref(), a.vararg.ty()) {
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
    if !b.vararg.is_present() {
        // Leftover source positionals must be optional or keyword-consumed.
        let unmet = a
            .positional
            .get(a_idx..)
            .unwrap_or(&[])
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
    if !b.vararg.is_present() {
        return true;
    }
    let bv = b.vararg.ty();
    for ap in a.positional.get(a_idx..).unwrap_or(&[]) {
        if consumed.contains(ap.name.as_str()) {
            continue;
        }
        if !ap.has_default || !ty_subtype(bv, ap.ty.as_deref()) {
            return false;
        }
    }
    a.vararg.is_present() && ty_subtype(bv, a.vararg.ty())
}

/// `**kwargs` compatibility, including unmatched source keyword-only params.
fn kwarg_compatible(a: &Sig, b: &Sig, consumed: &HashSet<&str>) -> bool {
    let unconsumed = a
        .kwonly
        .iter()
        .filter(|ak| !consumed.contains(ak.name.as_str()));
    if b.kwarg.is_present() {
        let bkw = b.kwarg.ty();
        if !a.kwarg.is_present() {
            return false;
        }
        if !ty_subtype(bkw, a.kwarg.ty()) {
            return false;
        }
        for ak in unconsumed {
            if !ak.has_default || !ty_subtype(bkw, ak.ty.as_deref()) {
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
    a.kwarg.is_present() && ty_subtype(bk.ty.as_deref(), a.kwarg.ty())
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
        None => a.kwarg.is_present() && ty_subtype(bk.ty.as_deref(), a.kwarg.ty()),
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
        None => b.kwarg.is_present() && ty_subtype(b.kwarg.ty(), ak.ty.as_deref()),
    }
}

// ---------------------------------------------------------------------------
// Type-text subtyping
// ---------------------------------------------------------------------------

/// Containers that are covariant in their element types.
const COVARIANT_BASES: &[&str] = &[
    "Sequence",
    "Iterable",
    "Iterator",
    "Collection",
    "tuple",
    "frozenset",
];

/// `true` when type text `narrow` is a subtype of `wide`.  Unannotated types
/// are treated as `Any` (compatible in both directions).
pub(super) fn ty_subtype(narrow: Option<&str>, wide: Option<&str>) -> bool {
    let (Some(narrow), Some(wide)) = (narrow, wide) else {
        return true;
    };
    let narrow = narrow.trim();
    let wide = wide.trim();
    if narrow == wide || narrow == "Any" || wide == "Any" || wide == "object" {
        return true;
    }
    let narrow_members = split_union(narrow);
    if narrow_members.len() > 1 {
        return narrow_members
            .iter()
            .all(|member| ty_subtype(Some(member), Some(wide)));
    }
    let wide_members = split_union(wide);
    if wide_members.len() > 1 {
        return wide_members
            .iter()
            .any(|member| ty_subtype(Some(narrow), Some(member)));
    }
    if is_numeric_subtype(narrow, wide) {
        return true;
    }
    covariant_container_subtype(narrow, wide)
}

/// `Sequence[float] <: Sequence[object]` — same covariant base, element-wise.
fn covariant_container_subtype(narrow: &str, wide: &str) -> bool {
    let (Some(narrow_base), Some(wide_base)) = (narrow.split('[').next(), wide.split('[').next())
    else {
        return false;
    };
    if narrow_base != wide_base || !COVARIANT_BASES.contains(&narrow_base.trim()) {
        return false;
    }
    let inner = |text: &str| -> Option<Vec<String>> {
        let start = text.find('[')?;
        let inner = text.get(start + 1..)?.strip_suffix(']')?;
        Some(
            split_top_level_commas(inner)
                .into_iter()
                .map(|s| s.trim().to_owned())
                .collect(),
        )
    };
    let (Some(narrow_args), Some(wide_args)) = (inner(narrow), inner(wide)) else {
        return false;
    };
    narrow_args.len() == wide_args.len()
        && narrow_args
            .iter()
            .zip(wide_args.iter())
            .all(|(narrow_arg, wide_arg)| ty_subtype(Some(narrow_arg), Some(wide_arg)))
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
