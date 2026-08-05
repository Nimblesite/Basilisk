//! Implements [TYPEINF-FUNC-SELFCLS] receiver binding for the
//! `calls_argument_count` method path. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//!
//! A method is a method however it was defined: a literal `def` in the class
//! body, or a module-level function bound by a class-body assignment
//! (`m = f`, `s = staticmethod(g)`, `c = classmethod(h)` —
//! [#382](https://github.com/Nimblesite/Basilisk/issues/382)). Instance access
//! (`C().m(...)`) consumes the implicit receiver; class access (`C.m(...)`)
//! does not; `staticmethod` never consumes one and `classmethod` always does.

use basilisk_resolver::scope::CallReceiver;
use basilisk_resolver::{CallSite, ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::super::shared;

/// Decorators that leave a method's call signature intact. Anything else
/// (`property`, custom descriptors, wrappers) may change what a call accepts,
/// so the arity check abstains rather than guess.
const SIGNATURE_PRESERVING: [&str; 2] = ["staticmethod", "classmethod"];

/// How a resolved class attribute binds its underlying callable.
struct BoundMethod<'a> {
    /// Candidate signatures (multiple for `@overload` groups or redefinitions);
    /// the call is accepted when ANY candidate accepts it.
    candidates: Vec<&'a FunctionInfo>,
    /// The `staticmethod` / `classmethod` wrapper applied by assignment, if any.
    wrapper: Option<&'a str>,
}

/// Check method calls through a class receiver — `C.m(...)` and `C().m(...)` —
/// against the bound method's signature, consuming the implicit receiver
/// according to the access path and any descriptor wrapper ([#382]).
pub(super) fn check_method_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let class_map = shared::class_name_map(&module.classes);
    let method_map = shared::method_name_map(&module.functions);

    for call in &module.calls {
        let Some((class_info, instance_access)) = receiver_class(call, &class_map) else {
            continue;
        };
        // Keyword arguments and `**kwargs` unpacking hide how many parameters
        // are satisfied; the positional-arity check abstains (same guard as
        // every other path in this rule).
        if !call.keywords.is_empty() || call.has_unpacked_kwargs {
            continue;
        }
        let Some(bound) = resolve_bound_method(module, class_info, &call.callee, &method_map)
        else {
            continue;
        };
        check_bound_call(
            module,
            call,
            class_info,
            &bound,
            instance_access,
            diagnostics,
        );
    }
}

/// The class a call's receiver denotes, and whether the access path goes
/// through an instance (`C().m` — `true`) or the class object (`C.m`).
fn receiver_class<'a>(
    call: &CallSite,
    class_map: &std::collections::HashMap<&str, &'a ClassInfo>,
) -> Option<(&'a ClassInfo, bool)> {
    match call.receiver.as_ref()? {
        CallReceiver::Name(name) => class_map.get(name.as_str()).map(|cls| (*cls, false)),
        CallReceiver::Constructor(name) => class_map.get(name.as_str()).map(|cls| (*cls, true)),
        CallReceiver::StringLiteral | CallReceiver::BytesLiteral => None,
    }
}

/// Resolve `class.method` to its candidate signatures: literal `def`s first,
/// else a class-body assignment binding a module-level function. Returns
/// `None` (abstain) when the method is unknown here or a decorator may have
/// changed its signature.
fn resolve_bound_method<'a>(
    module: &'a ResolvedModule,
    class_info: &'a ClassInfo,
    method: &str,
    method_map: &std::collections::HashMap<(&str, &str), Vec<&'a FunctionInfo>>,
) -> Option<BoundMethod<'a>> {
    if let Some(defs) = method_map.get(&(class_info.name.as_str(), method)) {
        let all_preserving = defs
            .iter()
            .all(|f| signature_preserving_decorators(&f.decorators));
        return all_preserving.then(|| BoundMethod {
            candidates: defs.clone(),
            wrapper: None,
        });
    }
    let attribute = class_info
        .attributes
        .iter()
        .find(|a| a.name == method && !a.has_annotation)?;
    let bound_name = attribute.rhs_name.as_deref()?;
    let candidates: Vec<&FunctionInfo> = module
        .functions
        .iter()
        .filter(|f| f.class_name.is_none() && !f.nested_in_class && f.name == bound_name)
        .filter(|f| signature_preserving_decorators(&f.decorators))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(BoundMethod {
        candidates,
        wrapper: attribute.rhs_descriptor.as_deref(),
    })
}

/// `true` when every decorator on a function is known to preserve its
/// signature, so the raw parameter list is what a call binds against.
fn signature_preserving_decorators(decorators: &[String]) -> bool {
    decorators.iter().all(|d| {
        let leaf = d.rsplit('.').next().unwrap_or(d.as_str());
        SIGNATURE_PRESERVING.contains(&leaf)
    })
}

/// How many leading parameters the descriptor protocol consumes for this
/// binding and access path: `staticmethod` none, `classmethod` its `cls` on
/// both paths, a plain function its `self` on instance access only.
fn receiver_params_consumed(
    func: &FunctionInfo,
    wrapper: Option<&str>,
    instance_access: bool,
) -> usize {
    let spelled =
        |name: &str| wrapper == Some(name) || shared::decorator_spelled(&func.decorators, name);
    if spelled("staticmethod") {
        return 0;
    }
    usize::from(spelled("classmethod") || instance_access)
}

/// The positional arguments a signature requires once `consumed` leading
/// parameters are bound, or `None` when `*args` makes any count acceptable.
fn required_after_binding(func: &FunctionInfo, consumed: usize) -> Option<usize> {
    func.vararg.is_none().then(|| {
        func.parameters
            .iter()
            .skip(consumed)
            .filter(|p| !p.has_default)
            .count()
    })
}

/// Emit a missing-argument diagnostic when no candidate signature accepts the
/// provided positional count under the binding's receiver consumption.
fn check_bound_call(
    module: &ResolvedModule,
    call: &CallSite,
    class_info: &ClassInfo,
    bound: &BoundMethod<'_>,
    instance_access: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let provided = call.args.len();
    let mut min_required = usize::MAX;
    for func in &bound.candidates {
        let consumed = receiver_params_consumed(func, bound.wrapper, instance_access);
        match required_after_binding(func, consumed) {
            None => return,
            Some(required) if provided >= required => return,
            Some(required) => min_required = min_required.min(required),
        }
    }
    let Some(missing) = min_required.checked_sub(provided).filter(|m| *m > 0) else {
        return;
    };
    let access = if instance_access {
        "the instance receiver is bound implicitly"
    } else {
        "accessing through the class binds no receiver, so the first argument fills it"
    };
    diagnostics.push(error_diagnostic_owned(
        super::CODE.clone(),
        format!(
            "Call to `{}.{}()` is missing {missing} required argument{} \
             (expected {min_required}, got {provided}; {access})",
            class_info.name,
            call.callee,
            if missing == 1 { "" } else { "s" },
        ),
        call.span,
        &module.path,
        None,
        None,
    ));
}
