//! BSK-E0140: Callable and Protocol assignment compatibility.
//!
//! Checks that when a function is assigned to a variable annotated with a
//! `Callable` type or a callback `Protocol`, the signatures are compatible.

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0140",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0140",
};

/// Emits BSK-E0140 for incompatible callable/protocol assignments.
pub(crate) struct CallableAssignmentViolation;

impl Rule for CallableAssignmentViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

#[derive(Debug, Clone)]
struct FuncSig {
    name: String,
    positional_params: Vec<ParamInfo>,
    has_varargs: bool,
    varargs_type: String,
    has_kwargs: bool,
    kwargs_type: String,
    kw_only_params: Vec<ParamInfo>,
    #[expect(dead_code, reason = "return_type will be used for future type checking")]
    return_type: String,
}

#[derive(Debug, Clone)]
struct ParamInfo {
    name: String,
    type_annotation: String,
    has_default: bool,
    is_positional_only: bool,
}

#[derive(Debug, Clone)]
struct ProtocolInfo {
    name: String,
    call_sig: Option<FuncSig>,
    has_extra_attrs: bool,
}

struct ModuleContext {
    functions: Vec<FuncSig>,
    protocols: Vec<ProtocolInfo>,
    non_protocol_classes: Vec<String>,
}

impl ModuleContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut functions = Vec::new();
        let mut protocols = Vec::new();
        let mut non_protocol_classes = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    functions.push(extract_func_sig(func, false));
                }
                Stmt::ClassDef(cls) => {
                    if is_protocol_class(cls) {
                        protocols.push(extract_protocol_info(cls));
                    } else if has_call_method(cls) {
                        non_protocol_classes.push(cls.name.to_string());
                    }
                }
                _ => {}
            }
        }
        Self {
            functions,
            protocols,
            non_protocol_classes,
        }
    }
    fn find_func(&self, name: &str) -> Option<&FuncSig> {
        self.functions.iter().find(|f| f.name == name)
    }
    fn find_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.iter().find(|p| p.name == name)
    }
    fn is_non_protocol_class(&self, name: &str) -> bool {
        self.non_protocol_classes.iter().any(|n| n == name)
    }
}

fn is_protocol_class(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|arg| match arg {
            Expr::Name(name) => name.id.as_str() == "Protocol",
            Expr::Subscript(sub) => {
                matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Protocol")
            }
            _ => false,
        })
    })
}

fn has_call_method(cls: &ast::StmtClassDef) -> bool {
    cls.body
        .iter()
        .any(|stmt| matches!(stmt, Stmt::FunctionDef(f) if f.name.as_str() == "__call__"))
}

fn extract_protocol_info(cls: &ast::StmtClassDef) -> ProtocolInfo {
    let mut call_sig = None;
    let mut has_extra_attrs = false;
    for body_stmt in &cls.body {
        match body_stmt {
            Stmt::FunctionDef(func) => {
                if func.name.as_str() == "__call__" && !is_overload_decorated(func) {
                    call_sig = Some(extract_func_sig(func, true));
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Some(attr_name) = expr_name(&ann.target) {
                    if !matches!(
                        attr_name,
                        "__name__" | "__module__" | "__qualname__" | "__annotations__" | "__doc__"
                    ) {
                        has_extra_attrs = true;
                    }
                }
            }
            _ => {}
        }
    }
    ProtocolInfo {
        name: cls.name.to_string(),
        call_sig,
        has_extra_attrs,
    }
}

fn is_overload_decorated(func: &ast::StmtFunctionDef) -> bool {
    func.decorator_list
        .iter()
        .any(|dec| matches!(&dec.expression, Expr::Name(n) if n.id.as_str() == "overload"))
}

fn extract_func_sig(func: &ast::StmtFunctionDef, skip_self: bool) -> FuncSig {
    let params = &func.parameters;
    let mut positional_params = Vec::new();
    let mut kw_only_params = Vec::new();
    for (idx, param) in params.posonlyargs.iter().enumerate() {
        if skip_self && idx == 0 && param.parameter.name.as_str() == "self" {
            continue;
        }
        positional_params.push(mk_param(param, true));
    }
    for (idx, param) in params.args.iter().enumerate() {
        if skip_self
            && positional_params.is_empty()
            && idx == 0
            && param.parameter.name.as_str() == "self"
        {
            continue;
        }
        positional_params.push(mk_param(param, false));
    }
    for param in &params.kwonlyargs {
        kw_only_params.push(mk_param(param, false));
    }
    let has_varargs = params.vararg.is_some();
    let varargs_type = params
        .vararg
        .as_ref()
        .and_then(|v| v.annotation.as_ref().map(|a| ann_str(a)))
        .unwrap_or_default();
    let has_kwargs = params.kwarg.is_some();
    let kwargs_type = params
        .kwarg
        .as_ref()
        .and_then(|k| k.annotation.as_ref().map(|a| ann_str(a)))
        .unwrap_or_default();
    let return_type = func
        .returns
        .as_ref()
        .map(|r| ann_str(r))
        .unwrap_or_default();
    FuncSig {
        name: func.name.to_string(),
        positional_params,
        has_varargs,
        varargs_type,
        has_kwargs,
        kwargs_type,
        kw_only_params,
        return_type,
    }
}

fn mk_param(param: &ast::ParameterWithDefault, is_pos_only: bool) -> ParamInfo {
    ParamInfo {
        name: param.parameter.name.to_string(),
        type_annotation: param
            .parameter
            .annotation
            .as_ref()
            .map(|a| ann_str(a))
            .unwrap_or_default(),
        has_default: param.default.is_some(),
        is_positional_only: is_pos_only,
    }
}

fn check_stmts(stmts: &[Stmt], ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    let mut annotations: Vec<(String, Expr)> = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    annotations.push((name.to_owned(), (*ann.annotation).clone()));
                }
                if let Some(value) = &ann.value {
                    let span = Span {
                        start: ann.range().start().to_u32(),
                        end: ann.range().end().to_u32(),
                    };
                    check_assignment(&ann.annotation, value, ctx, path, diag, span);
                }
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Some(target_name) = expr_name(&assign.targets[0]) {
                        if let Some((_, prev_ann)) =
                            annotations.iter().rev().find(|(n, _)| n == target_name)
                        {
                            let span = Span {
                                start: assign.range().start().to_u32(),
                                end: assign.range().end().to_u32(),
                            };
                            check_assignment(prev_ann, &assign.value, ctx, path, diag, span);
                        }
                    }
                }
            }
            Stmt::FunctionDef(func) => check_stmts_in_func(&func.body, ctx, path, diag),
            Stmt::ClassDef(cls) => check_stmts(&cls.body, ctx, path, diag),
            _ => {}
        }
    }
}

fn check_stmts_in_func(
    stmts: &[Stmt],
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let mut local_annotations: Vec<(String, Expr)> = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    local_annotations.push((name.to_owned(), (*ann.annotation).clone()));
                }
                if let Some(value) = &ann.value {
                    let span = Span {
                        start: ann.range().start().to_u32(),
                        end: ann.range().end().to_u32(),
                    };
                    check_assignment(&ann.annotation, value, ctx, path, diag, span);
                }
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Some(target_name) = expr_name(&assign.targets[0]) {
                        if let Some((_, prev_ann)) = local_annotations
                            .iter()
                            .rev()
                            .find(|(n, _)| n == target_name)
                        {
                            let span = Span {
                                start: assign.range().start().to_u32(),
                                end: assign.range().end().to_u32(),
                            };
                            check_assignment(prev_ann, &assign.value, ctx, path, diag, span);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn check_assignment(
    annotation: &Expr,
    value: &Expr,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let value_name = expr_name(value);
    let ann_s = ann_str(annotation);

    // Callable[...] annotation
    if ann_s.starts_with("Callable[") {
        if let Some(cinfo) = parse_callable_type(&ann_s) {
            if let Some(fname) = value_name {
                if let Some(fsig) = ctx.find_func(fname) {
                    check_callable_compat(&cinfo, fsig, &ann_s, path, diag, span);
                }
            }
        }
        return;
    }

    // Protocol type annotation
    let base = extract_base_name(&ann_s);
    if ctx.is_non_protocol_class(&base) {
        if let Some(fname) = value_name {
            if ctx.find_func(fname).is_some() {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot assign function `{fname}` to non-protocol type `{base}`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                });
            }
        }
        return;
    }
    if let Some(protocol) = ctx.find_protocol(&base) {
        if let Some(fname) = value_name {
            if let Some(fsig) = ctx.find_func(fname) {
                check_protocol_func_compat(protocol, fsig, path, diag, span);
            }
        }
    }
}

#[expect(dead_code, reason = "struct will be used for future type checking")]
struct CallableTypeInfo {
    param_types: Option<Vec<String>>,
    concatenate_prefix: Vec<String>,
    is_open_ended: bool,
    #[expect(dead_code, reason = "return_type will be used for future type checking")]
    return_type: String,
}

fn parse_callable_type(s: &str) -> Option<CallableTypeInfo> {
    if !s.starts_with("Callable[") {
        return None;
    }
    let inner = &s["Callable[".len()..s.len().checked_sub(1)?];
    let (first, ret) = split_top_comma(inner)?;
    let first = first.trim();
    let ret = ret.trim().to_owned();
    if first == "..." {
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: Vec::new(),
            is_open_ended: true,
            return_type: ret,
        });
    }
    if first.starts_with("Concatenate[") {
        let ci = &first["Concatenate[".len()..first.len().checked_sub(1)?];
        let parts = split_all_commas(ci);
        let mut prefix = Vec::new();
        let mut open = false;
        for p in &parts {
            let p = p.trim();
            if p == "..." {
                open = true;
            } else {
                prefix.push(p.to_owned());
            }
        }
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: prefix,
            is_open_ended: open,
            return_type: ret,
        });
    }
    if first.starts_with('[') && first.ends_with(']') {
        let li = &first[1..first.len() - 1];
        let types = if li.trim().is_empty() {
            Vec::new()
        } else {
            split_all_commas(li)
                .iter()
                .map(|s| s.trim().to_owned())
                .collect()
        };
        return Some(CallableTypeInfo {
            param_types: Some(types),
            concatenate_prefix: Vec::new(),
            is_open_ended: false,
            return_type: ret,
        });
    }
    None
}

fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut d: usize = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

fn split_all_commas(s: &str) -> Vec<&str> {
    let mut d: usize = 0;
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

fn check_callable_compat(
    ci: &CallableTypeInfo,
    func: &FuncSig,
    ann: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if !ci.concatenate_prefix.is_empty() {
        let req = ci.concatenate_prefix.len();
        let fpos = func.positional_params.len();
        if fpos == 0 && !func.kw_only_params.is_empty() {
            diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                message: format!("Function `{}` incompatible with `{ann}`: Concatenate requires positional params", func.name),
                span, path: path.to_owned(), help: None, note: None });
            return;
        }
        if fpos < req {
            diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                message: format!("Function `{}` incompatible with `{ann}`: needs at least {req} positional param(s) but has {fpos}", func.name),
                span, path: path.to_owned(), help: None, note: None });
            return;
        }
        for (idx, exp) in ci.concatenate_prefix.iter().enumerate() {
            if idx < func.positional_params.len() {
                let act = &func.positional_params[idx].type_annotation;
                if !act.is_empty() && !types_compat(exp, act) {
                    diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                        message: format!("Function `{}` incompatible with `{ann}`: param {} type `{act}` vs required `{exp}`", func.name, idx+1),
                        span, path: path.to_owned(), help: None, note: None });
                }
            }
        }
        return;
    }
    if let Some(ptypes) = &ci.param_types {
        let exp = ptypes.len();
        let min = func
            .positional_params
            .iter()
            .filter(|p| !p.has_default)
            .count();
        let max = func.positional_params.len();
        if exp < min {
            diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                message: format!("Function `{}` incompatible with `{ann}`: callable provides {exp} args but function requires {min}", func.name),
                span, path: path.to_owned(), help: None, note: None });
        } else if exp > max && !func.has_varargs {
            diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                message: format!("Function `{}` incompatible with `{ann}`: callable provides {exp} args but function accepts {max}", func.name),
                span, path: path.to_owned(), help: None, note: None });
        }
    }
}

fn check_protocol_func_compat(
    proto: &ProtocolInfo,
    func: &FuncSig,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if proto.has_extra_attrs {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` cannot satisfy protocol `{}`: protocol has extra attributes",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return;
    }
    let Some(target) = &proto.call_sig else {
        return;
    };

    if check_protocol_varargs_kwargs(target, func, proto, path, diag, span) {
        return;
    }
    if check_protocol_param_counts(target, func, proto, path, diag, span) {
        return;
    }
    check_protocol_defaults_and_kw(target, func, proto, path, diag, span);
    check_protocol_param_types(target, func, proto, path, diag, span);
}

/// Check *args and **kwargs compatibility. Returns `true` if a fatal mismatch
/// was found and the caller should stop further checks.
fn check_protocol_varargs_kwargs(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) -> bool {
    if target.has_varargs && !func.has_varargs && target.positional_params.is_empty() {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing `*args`",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    if target.has_kwargs && !func.has_kwargs {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing `**kwargs`",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    false
}

/// Check positional parameter count compatibility. Returns `true` if a fatal
/// mismatch was found.
fn check_protocol_param_counts(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) -> bool {
    let src_req = func
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    if src_req > target.positional_params.len() && !target.has_varargs {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: too many required params",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    let tgt_req = target
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    if tgt_req > func.positional_params.len() && !func.has_varargs {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing required params",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    false
}

/// Check default-argument requirements, keyword-only params, and positional-only
/// mismatches.
fn check_protocol_defaults_and_kw(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    // Default arg check
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if tp.has_default {
            if let Some(sp) = func.positional_params.get(idx) {
                if !sp.has_default && !func.has_varargs {
                    diag.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Function `{}` incompatible with `{}`: param `{}` needs default",
                            func.name, proto.name, sp.name
                        ),
                        span,
                        path: path.to_owned(),
                        help: None,
                        note: None,
                    });
                }
            }
        }
    }
    // Keyword-only params
    for tkw in &target.kw_only_params {
        let has_kw = func.kw_only_params.iter().any(|sk| sk.name == tkw.name);
        let has_reg = func
            .positional_params
            .iter()
            .any(|sp| sp.name == tkw.name && !sp.is_positional_only);
        if !has_kw && !has_reg && !func.has_kwargs {
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{}`: missing keyword param `{}`",
                    func.name, proto.name, tkw.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
        }
    }
    // Positional-only mismatch
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if !tp.is_positional_only {
            if let Some(sp) = func.positional_params.get(idx) {
                if sp.is_positional_only {
                    diag.push(Diagnostic { code: CODE.clone(), severity: Severity::Error,
                        message: format!("Function `{}` incompatible with `{}`: param `{}` is pos-only but must accept keyword", func.name, proto.name, sp.name),
                        span, path: path.to_owned(), help: None, note: None });
                }
            }
        }
    }
}

/// Check parameter type compatibility (contravariant), *args type, and **kwargs
/// type.
fn check_protocol_param_types(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    // Param type compat (contravariant)
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if let Some(sp) = func.positional_params.get(idx) {
            if !tp.type_annotation.is_empty()
                && !sp.type_annotation.is_empty()
                && !types_compat(&tp.type_annotation, &sp.type_annotation)
            {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` incompatible with `{}`: param `{}` type `{}` vs `{}`",
                        func.name, proto.name, sp.name, sp.type_annotation, tp.type_annotation
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                });
            }
        }
    }
    // *args type compat
    if target.has_varargs
        && func.has_varargs
        && !target.varargs_type.is_empty()
        && !func.varargs_type.is_empty()
        && !types_compat(&target.varargs_type, &func.varargs_type)
    {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: *args type `{}` vs `{}`",
                func.name, proto.name, func.varargs_type, target.varargs_type
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    }
    // **kwargs type compat
    if target.has_kwargs
        && func.has_kwargs
        && !target.kwargs_type.is_empty()
        && !func.kwargs_type.is_empty()
        && !types_compat(&target.kwargs_type, &func.kwargs_type)
    {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: **kwargs type `{}` vs `{}`",
                func.name, proto.name, func.kwargs_type, target.kwargs_type
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    }
}

fn types_compat(target: &str, source: &str) -> bool {
    if target == source {
        return true;
    }
    if target == "Any" || source == "Any" {
        return true;
    }
    if target.is_empty() || source.is_empty() {
        return true;
    }
    if target == "int" && source == "float" {
        return true;
    }
    if target == "float" && source == "int" {
        return true;
    }
    if target == "bool" && source == "int" {
        return true;
    }
    if target.contains(" | ") {
        return target.split(" | ").any(|m| m.trim() == source);
    }
    let builtins = [
        "int", "str", "float", "bool", "bytes", "None", "complex", "object",
    ];
    if builtins.contains(&target) && builtins.contains(&source) {
        return false;
    }
    true
}

fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

fn extract_base_name(s: &str) -> String {
    s.find('[')
        .map_or_else(|| s.trim().to_owned(), |i| s[..i].trim().to_owned())
}

fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::List(l) => format!(
            "[{}]",
            l.elts.iter().map(ann_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::NumberLiteral(n) => format!("{:?}", n.value),
        _ => "...".to_owned(),
    }
}
