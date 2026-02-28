//! AST visitor that collects function definitions.

use ruff_python_ast::{
    ElifElseClause, ExceptHandler, Parameter, ParameterWithDefault, Stmt, StmtFunctionDef,
};
use ruff_text_size::TextRange;

use basilisk_parser::ParsedModule;

use crate::{
    scope::{FunctionInfo, ParameterInfo, ResolvedModule, Span},
    ResolveError,
};

/// Collect all function definitions from the parsed module.
pub(crate) fn collect_functions(module: &ParsedModule) -> Result<ResolvedModule, ResolveError> {
    let mut functions = Vec::new();
    collect_from_body(&module.ast.body, &mut functions);

    Ok(ResolvedModule {
        functions,
        path: module.path.clone(),
        source: module.source.clone(),
    })
}

fn collect_from_body(stmts: &[Stmt], out: &mut Vec<FunctionInfo>) {
    stmts.iter().for_each(|stmt| collect_from_stmt(stmt, out));
}

fn collect_from_stmt(stmt: &Stmt, out: &mut Vec<FunctionInfo>) {
    match stmt {
        Stmt::FunctionDef(func) => {
            out.push(function_info_from(func));
            collect_from_body(&func.body, out);
        }
        Stmt::ClassDef(class) => {
            collect_from_body(&class.body, out);
        }
        Stmt::If(node) => {
            collect_from_body(&node.body, out);
            // In ruff 0.12.x, StmtIf uses `elif_else_clauses` instead of `orelse`
            collect_from_elif_else(&node.elif_else_clauses, out);
        }
        Stmt::For(node) => {
            collect_from_body(&node.body, out);
            collect_from_body(&node.orelse, out);
        }
        Stmt::While(node) => {
            collect_from_body(&node.body, out);
            collect_from_body(&node.orelse, out);
        }
        Stmt::With(node) => {
            collect_from_body(&node.body, out);
        }
        Stmt::Try(node) => {
            collect_from_body(&node.body, out);
            collect_from_handlers(&node.handlers, out);
            collect_from_body(&node.orelse, out);
            collect_from_body(&node.finalbody, out);
        }
        _ => {}
    }
}

fn collect_from_elif_else(clauses: &[ElifElseClause], out: &mut Vec<FunctionInfo>) {
    clauses
        .iter()
        .for_each(|clause| collect_from_body(&clause.body, out));
}

fn collect_from_handlers(handlers: &[ExceptHandler], out: &mut Vec<FunctionInfo>) {
    for handler in handlers {
        let ExceptHandler::ExceptHandler(h) = handler;
        collect_from_body(&h.body, out);
    }
}

fn function_info_from(func: &StmtFunctionDef) -> FunctionInfo {
    let params = &func.parameters;

    let positional: Vec<ParameterInfo> = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .map(param_with_default_to_info)
        .collect();

    let kwonly: Vec<ParameterInfo> = params
        .kwonlyargs
        .iter()
        .map(param_with_default_to_info)
        .collect();

    let all_params: Vec<ParameterInfo> = positional.into_iter().chain(kwonly).collect();

    let vararg = params.vararg.as_deref().map(parameter_to_info);
    let kwarg = params.kwarg.as_deref().map(parameter_to_info);

    FunctionInfo {
        name: func.name.to_string(),
        parameters: all_params,
        vararg,
        kwarg,
        has_return_annotation: func.returns.is_some(),
        def_span: text_range_to_span(func.range),
        name_span: text_range_to_span(func.name.range),
    }
}

fn param_with_default_to_info(p: &ParameterWithDefault) -> ParameterInfo {
    parameter_to_info(&p.parameter)
}

fn parameter_to_info(p: &Parameter) -> ParameterInfo {
    ParameterInfo {
        name: p.name.to_string(),
        has_annotation: p.annotation.is_some(),
        name_span: text_range_to_span(p.name.range),
    }
}

fn text_range_to_span(range: TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
