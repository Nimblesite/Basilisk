//! Constructor-to-callable synthesis for [STUBRES-PYI] #289.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ParameterInfo};

/// Method-map key: `(class_name, method_name)` to declarations in source order.
pub(super) type MethodMap<'a> = HashMap<(&'a str, &'a str), Vec<&'a FunctionInfo>>;

/// One overload in the callable union synthesized from a constructor.
pub(super) struct CallableVariant<'a> {
    /// Parameters after binding away `cls` or `self`.
    pub(super) params: Vec<&'a ParameterInfo>,
    /// Whether this alternative accepts arbitrary positional arguments.
    pub(super) has_var_positional: bool,
    /// Whether this alternative accepts arbitrary keyword arguments.
    pub(super) has_var_keyword: bool,
}

/// Synthesize every applicable bound constructor signature.
///
/// A special metaclass `__call__` terminates conversion. Otherwise the result
/// is the union of inherited non-`object` `__new__` and `__init__` signatures;
/// a non-instance `__new__` return terminates before `__init__`. Classes with
/// neither method use the zero-argument `object` fallback.
pub(super) fn build_converted_callables<'a>(
    class_name: &str,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    method_map: &MethodMap<'a>,
    source: &str,
) -> Vec<CallableVariant<'a>> {
    if let Some(metaclass) = class_map
        .get(class_name)
        .and_then(|class| class.metaclass_name.as_deref())
    {
        let calls = inherited_methods(metaclass, "__call__", class_map, method_map);
        if calls
            .iter()
            .any(|method| return_is_non_instance(method, class_name, class_map, source))
        {
            return calls.into_iter().map(bound_variant).collect();
        }
    }

    let news = inherited_methods(class_name, "__new__", class_map, method_map);
    let new_terminates = news
        .iter()
        .any(|method| return_is_non_instance(method, class_name, class_map, source));
    let mut variants: Vec<_> = news.into_iter().map(bound_variant).collect();
    if new_terminates {
        return variants;
    }

    variants.extend(
        inherited_methods(class_name, "__init__", class_map, method_map)
            .into_iter()
            .map(bound_variant),
    );
    if variants.is_empty() {
        variants.push(CallableVariant {
            params: Vec::new(),
            has_var_positional: false,
            has_var_keyword: false,
        });
    }
    variants
}

/// Find the first class in the C3 MRO defining `method`, preserving all of its
/// overload declarations and discarding a permissive implementation signature.
fn inherited_methods<'a>(
    class_name: &str,
    method: &str,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    method_map: &MethodMap<'a>,
) -> Vec<&'a FunctionInfo> {
    crate::stub_constructor::mro_over(class_name, &|name| {
        class_map.get(name).map_or_else(Vec::new, |class| {
            class
                .bases
                .iter()
                .map(|base| crate::stub_constructor::base_head(base).to_owned())
                .filter(|base| base != "object" && base != "Any")
                .collect()
        })
    })
    .into_iter()
    .find_map(|class| {
        method_map
            .get(&(class.as_str(), method))
            .map(|methods| declared_signatures(methods))
    })
    .unwrap_or_default()
}

/// Overloads are the callable contract; the implementation is used only when
/// no overload declarations exist.
fn declared_signatures<'a>(methods: &[&'a FunctionInfo]) -> Vec<&'a FunctionInfo> {
    let overloads: Vec<_> = methods
        .iter()
        .copied()
        .filter(|method| is_overload(method))
        .collect();
    if overloads.is_empty() {
        methods
            .iter()
            .copied()
            .find(|method| !is_overload(method))
            .or_else(|| methods.first().copied())
            .into_iter()
            .collect()
    } else {
        overloads
    }
}

fn is_overload(method: &FunctionInfo) -> bool {
    method.decorators.iter().any(|decorator| {
        decorator == "overload" || decorator.rsplit('.').next() == Some("overload")
    })
}

/// Bind the constructor receiver to the class object/instance.
fn bound_variant(method: &FunctionInfo) -> CallableVariant<'_> {
    CallableVariant {
        params: method.parameters.iter().skip(1).collect(),
        has_var_positional: method.vararg.is_some(),
        has_var_keyword: method.kwarg.is_some(),
    }
}

/// Whether the declared return contains a type that is not the constructed
/// class (or one of its subclasses). Missing annotations are instance-like.
fn return_is_non_instance(
    method: &FunctionInfo,
    class_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    source: &str,
) -> bool {
    method
        .return_annotation_span
        .and_then(|span| span.slice_source(source))
        .is_some_and(|annotation| {
            union_members(annotation)
                .into_iter()
                .any(|member| !is_instance_type(member, class_name, class_map))
        })
}

/// Split PEP 604 and `Union[...]` spellings used by constructor returns.
fn union_members(annotation: &str) -> Vec<&str> {
    let annotation = annotation.trim().trim_matches(['\'', '"']);
    let union_body = annotation
        .strip_prefix("Union[")
        .or_else(|| annotation.strip_prefix("typing.Union["))
        .and_then(|body| body.strip_suffix(']'));
    union_body.map_or_else(
        || annotation.split('|').map(str::trim).collect(),
        |body| body.split(',').map(str::trim).collect(),
    )
}

fn is_instance_type(
    annotation: &str,
    class_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    let annotation = annotation.trim().trim_matches(['\'', '"']);
    if annotation.rsplit('.').next() == Some("Self") {
        return true;
    }
    let head = crate::stub_constructor::base_head(annotation);
    let name = head.rsplit('.').next().unwrap_or(head);
    if name == class_name {
        return true;
    }
    class_map.get(name).is_some_and(|_| {
        crate::stub_constructor::mro_over(name, &|candidate| {
            class_map.get(candidate).map_or_else(Vec::new, |class| {
                class
                    .bases
                    .iter()
                    .map(|base| crate::stub_constructor::base_head(base).to_owned())
                    .collect()
            })
        })
        .iter()
        .any(|base| base == class_name)
    })
}
