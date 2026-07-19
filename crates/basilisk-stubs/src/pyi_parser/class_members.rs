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
            StubClass {
                name: class.name.to_string(),
                bases,
                metaclass,
                methods,
                attributes,
            },
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
        let alternatives: Vec<_> = feasible_branches(if_stmt, self.target.as_ref())
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
                self.overloads
                    .entry(qualified)
                    .or_default()
                    .push(method.clone());
            } else {
                let _ = self.functions.insert(qualified, method.clone());
            }
        }
    }
}
