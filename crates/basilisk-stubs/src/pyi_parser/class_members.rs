//! Class declaration and guarded-member extraction.

use ruff_python_ast::{Stmt, StmtClassDef};

use crate::types::{StubClass, StubFunction, StubVariable};

use super::guard::feasible_branches;
use super::syntax::{ann_assign_target_name, expr_to_annotation, stub_method};
use super::{retain_common, retain_common_by, same_stub_function, StubExtractor};

impl StubExtractor {
    pub(super) fn visit_class(&mut self, class: &StmtClassDef) {
        let bases = class
            .arguments
            .as_ref()
            .map(|arguments| arguments.args.iter().map(expr_to_annotation).collect())
            .unwrap_or_default();
        let metaclass = class.arguments.as_ref().and_then(|arguments| {
            arguments
                .keywords
                .iter()
                .find(|keyword| {
                    keyword
                        .arg
                        .as_ref()
                        .is_some_and(|name| name.as_str() == "metaclass")
                })
                .map(|keyword| expr_to_annotation(&keyword.value))
        });
        let (methods, attributes) = self.extract_class_members(&class.body, class.name.as_str());
        self.visit_function_methods_for_class(&class.name, &methods);
        let _ = self.classes.insert(
            class.name.to_string(),
            std::sync::Arc::new(StubClass {
                name: class.name.to_string(),
                bases,
                metaclass,
                methods,
                attributes,
            }),
        );
    }

    fn extract_class_members(
        &self,
        stmts: &[Stmt],
        class_name: &str,
    ) -> (Vec<StubFunction>, Vec<StubVariable>) {
        let mut methods = Vec::new();
        let mut attributes = Vec::new();
        self.collect_class_members(stmts, class_name, &mut methods, &mut attributes);
        (methods, attributes)
    }

    fn collect_class_members(
        &self,
        stmts: &[Stmt],
        class_name: &str,
        methods: &mut Vec<StubFunction>,
        attributes: &mut Vec<StubVariable>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(function) => methods.push(stub_method(function, class_name)),
                Stmt::AnnAssign(annotation) => {
                    if let Some(name) = ann_assign_target_name(annotation) {
                        attributes.push(StubVariable {
                            name,
                            annotation: Some(expr_to_annotation(&annotation.annotation)),
                        });
                    }
                }
                Stmt::If(if_stmt) => {
                    self.collect_guarded_class_members(if_stmt, class_name, methods, attributes);
                }
                _ => {}
            }
        }
    }

    fn collect_guarded_class_members(
        &self,
        if_stmt: &ruff_python_ast::StmtIf,
        class_name: &str,
        methods: &mut Vec<StubFunction>,
        attributes: &mut Vec<StubVariable>,
    ) {
        let branches = feasible_branches(if_stmt, self.target.as_ref());
        // Fast path for the dominant `if sys.version_info >= (3, X): …` shape
        // (one guarded body, implicit empty else): collect the body straight
        // into the class vectors, then keep an addition only when the members
        // declared BEFORE the guard already contain an identical declaration —
        // exactly what intersecting `pre + additions` with the untouched `pre`
        // computes, without cloning everything collected so far for each
        // guard. Big stdlib classes carry dozens of version guards, so the
        // clone-based general path made class extraction quadratic.
        if let [Some(body), None] = branches.as_slice() {
            let method_start = methods.len();
            let attribute_start = attributes.len();
            self.collect_class_members(body, class_name, methods, attributes);
            retain_additions_declared_before(methods, method_start, same_stub_function);
            retain_additions_declared_before(attributes, attribute_start, |left, right| {
                left == right
            });
            return;
        }
        let alternatives: Vec<_> = branches
            .into_iter()
            .map(|body| {
                let mut branch_methods = methods.clone();
                let mut branch_attributes = attributes.clone();
                if let Some(stmts) = body {
                    self.collect_class_members(
                        stmts,
                        class_name,
                        &mut branch_methods,
                        &mut branch_attributes,
                    );
                }
                (branch_methods, branch_attributes)
            })
            .collect();
        let Some((first_methods, first_attributes)) = alternatives.first() else {
            return;
        };
        methods.clone_from(first_methods);
        attributes.clone_from(first_attributes);
        for (branch_methods, branch_attributes) in alternatives.iter().skip(1) {
            retain_common_by(methods, branch_methods, same_stub_function);
            retain_common(attributes, branch_attributes);
        }
    }

    fn visit_function_methods_for_class(&mut self, class_name: &str, methods: &[StubFunction]) {
        for method in methods {
            let qualified = format!("{class_name}.{}", method.name);
            if method.is_overload {
                std::sync::Arc::make_mut(self.overloads.entry(qualified).or_default())
                    .push(method.clone());
            } else {
                let _ = self
                    .functions
                    .insert(qualified, std::sync::Arc::new(method.clone()));
            }
        }
    }
}

/// Keep an addition appended at or after `start` only when the members
/// declared before `start` already contain a matching declaration — the
/// clone-free equivalent of intersecting `pre + additions` with `pre`.
fn retain_additions_declared_before<T>(
    items: &mut Vec<T>,
    start: usize,
    matches: impl Fn(&T, &T) -> bool,
) {
    let additions = items.split_off(start);
    let kept: Vec<T> = additions
        .into_iter()
        .filter(|addition| items.iter().any(|declared| matches(declared, addition)))
        .collect();
    items.extend(kept);
}
