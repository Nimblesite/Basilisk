//! Pins for [RESOLV-CANONICAL-BINDING] event ordering.
//!
//! Python creates a binding AFTER evaluating the statement that makes it
//! (<https://docs.python.org/3/reference/executionmodel.html#binding-of-names>):
//! an assignment's RHS, a class's bases, and a function's decorators all
//! evaluate against the PRECEDING binding of the name. A table that
//! timestamps the new binding at the statement's start resolves those uses
//! to the name currently being defined — a use the interpreter would never
//! see.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use basilisk_canonical::BindingTable;
use ruff_python_ast::{Expr, ModModule, Stmt};

fn parsed(source: &str) -> ModModule {
    ruff_python_parser::parse_module(source)
        .expect("test source must parse")
        .into_syntax()
}

#[test]
fn assignment_rhs_use_sees_the_preceding_binding() {
    let module = parsed(
        r"
from typing import Final as sealing_marker
sealing_marker = sealing_marker
",
    );
    let table = BindingTable::from_module(&module.body);
    let Some(Stmt::Assign(assign)) = module.body.last() else {
        panic!("last statement must be the assignment");
    };
    assert!(
        table.resolves_to(&assign.value, "typing", "Final"),
        "the renamed RHS evaluates before the assignment replaces its binding"
    );
}

#[test]
fn class_base_use_sees_the_preceding_binding() {
    let module = parsed(
        r"
from typing import Protocol as structural_contract

class structural_contract(structural_contract): ...
",
    );
    let table = BindingTable::from_module(&module.body);
    let Some(Stmt::ClassDef(class)) = module.body.last() else {
        panic!("last statement must be the class definition");
    };
    let base = class
        .arguments
        .as_ref()
        .and_then(|arguments| arguments.args.first())
        .expect("class must have one base");
    assert!(
        table.resolves_to(base, "typing", "Protocol"),
        "the renamed base evaluates before the class definition replaces its binding"
    );
}

#[test]
fn decorator_use_sees_the_preceding_binding() {
    let module = parsed(
        r"
from typing import final as seal_callable

@seal_callable
def seal_callable(): ...
",
    );
    let table = BindingTable::from_module(&module.body);
    let Some(Stmt::FunctionDef(function)) = module.body.last() else {
        panic!("last statement must be the function definition");
    };
    let decorator = &function
        .decorator_list
        .first()
        .expect("function must have one decorator")
        .expression;
    assert!(
        table.resolves_to(decorator, "typing", "final"),
        "the renamed decorator evaluates before the function definition replaces its binding"
    );
}

#[test]
fn use_after_the_statement_sees_the_new_binding() {
    let module = parsed(
        r"
from typing import Final as sealing_marker
sealing_marker = 3
survey_revision: sealing_marker = 1
",
    );
    let table = BindingTable::from_module(&module.body);
    let Some(Stmt::AnnAssign(ann)) = module.body.last() else {
        panic!("last statement must be the annotated assignment");
    };
    assert!(
        !table.resolves_to(&ann.annotation, "typing", "Final"),
        "after `F = 3` the name refers to the assignment, not the import"
    );
}

#[test]
fn for_target_is_bound_inside_the_loop_body() {
    let module = parsed(
        r"
for item in [1]:
    use = item
",
    );
    let table = BindingTable::from_module(&module.body);
    let Some(Stmt::For(node)) = module.body.first() else {
        panic!("first statement must be the for loop");
    };
    let Some(Stmt::Assign(assign)) = node.body.first() else {
        panic!("loop body must be the assignment");
    };
    let Expr::Name(_) = assign.value.as_ref() else {
        panic!("assignment RHS must be a name");
    };
    assert!(
        table.refers_to_local_definition(&assign.value),
        "the loop target binds when iteration starts — before the body \
         runs — so a body use refers to it"
    );
}
