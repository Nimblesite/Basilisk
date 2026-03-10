//! Call Hierarchy handlers (prepare, incoming, outgoing).
//!
//! Implements `textDocument/prepareCallHierarchy`, `callHierarchy/incomingCalls`,
//! and `callHierarchy/outgoingCalls` for the Basilisk LSP.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Range, SymbolKind, Url,
};

use crate::util::{find_symbol_at_offset, span_to_range, SymbolHit};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check whether `inner` is fully contained within `outer`.
fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Build a `CallHierarchyItem` for a function definition.
fn function_item(
    func: &basilisk_resolver::FunctionInfo,
    source: &str,
    uri: &Url,
) -> CallHierarchyItem {
    CallHierarchyItem {
        name: func.name.clone(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: uri.clone(),
        range: span_to_range(source, func.def_span),
        selection_range: span_to_range(source, func.name_span),
        data: None,
    }
}

/// Build a `CallHierarchyItem` for a class definition.
fn class_item(class: &basilisk_resolver::ClassInfo, source: &str, uri: &Url) -> CallHierarchyItem {
    CallHierarchyItem {
        name: class.name.clone(),
        kind: SymbolKind::CLASS,
        tags: None,
        detail: None,
        uri: uri.clone(),
        range: span_to_range(source, class.def_span),
        selection_range: span_to_range(source, class.name_span),
        data: None,
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Prepare call hierarchy: find the function or class at the given byte offset.
#[must_use]
pub fn prepare(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
) -> Vec<CallHierarchyItem> {
    let Some(hit) = find_symbol_at_offset(resolved, byte_offset) else {
        return vec![];
    };
    match hit {
        SymbolHit::Function(func) => vec![function_item(func, source, uri)],
        SymbolHit::Class(class) => vec![class_item(class, source, uri)],
        _ => vec![],
    }
}

/// Find incoming calls: which functions call `item_name`?
///
/// Iterates all call sites in the module, filters those where `callee == item_name`,
/// determines the enclosing function for each call, and groups by caller.
#[must_use]
pub fn incoming_calls(
    resolved: &ResolvedModule,
    source: &str,
    item_name: &str,
    uri: &Url,
) -> Vec<CallHierarchyIncomingCall> {
    // Collect all call sites where callee matches item_name.
    let matching_calls: Vec<_> = resolved
        .calls
        .iter()
        .filter(|call| call.callee == item_name)
        .collect();

    if matching_calls.is_empty() {
        return vec![];
    }

    // Group by enclosing function name.
    let mut grouped: HashMap<String, Vec<Range>> = HashMap::new();
    for call in &matching_calls {
        // Find the enclosing function whose def_span contains the call.
        if let Some(enclosing) = resolved
            .functions
            .iter()
            .find(|f| span_contains(f.def_span, call.span))
        {
            grouped
                .entry(enclosing.name.clone())
                .or_default()
                .push(span_to_range(source, call.span));
        }
    }

    // Build incoming call items.
    grouped
        .into_iter()
        .filter_map(|(caller_name, from_ranges)| {
            let caller_func = resolved.functions.iter().find(|f| f.name == caller_name)?;
            Some(CallHierarchyIncomingCall {
                from: function_item(caller_func, source, uri),
                from_ranges,
            })
        })
        .collect()
}

/// Find outgoing calls: which functions does `item_name` call?
///
/// Finds the function with the given name, then collects all call sites within
/// its `def_span` and groups them by callee.
#[must_use]
pub fn outgoing_calls(
    resolved: &ResolvedModule,
    source: &str,
    item_name: &str,
    uri: &Url,
) -> Vec<CallHierarchyOutgoingCall> {
    // Find the function definition for item_name.
    let Some(func) = resolved.functions.iter().find(|f| f.name == item_name) else {
        return vec![];
    };

    // Collect all call sites within this function's def_span.
    let mut grouped: HashMap<String, Vec<Range>> = HashMap::new();
    for call in &resolved.calls {
        if span_contains(func.def_span, call.span) {
            grouped
                .entry(call.callee.clone())
                .or_default()
                .push(span_to_range(source, call.span));
        }
    }

    // Build outgoing call items.
    grouped
        .into_iter()
        .filter_map(|(callee_name, from_ranges)| {
            // Try to find the callee as a function first, then as a class.
            let item = resolved
                .functions
                .iter()
                .find(|f| f.name == callee_name)
                .map(|f| function_item(f, source, uri))
                .or_else(|| {
                    resolved
                        .classes
                        .iter()
                        .find(|c| c.name == callee_name)
                        .map(|c| class_item(c, source, uri))
                });
            let item = item?;
            Some(CallHierarchyOutgoingCall {
                to: item,
                from_ranges,
            })
        })
        .collect()
}
