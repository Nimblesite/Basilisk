//! BSK-E0146: Protocol class object violations.
//!
//! Detects two related violations involving Protocol classes and class objects:
//!
//! 1. A Protocol class itself is passed/assigned where `type[Proto]` is expected.
//!    Only concrete (non-Protocol) subtypes may be used.
//!
//! 2. A class object is assigned to a variable typed as a Protocol instance,
//!    but the class does not structurally satisfy the protocol when treated as
//!    an object (i.e. class-level access to protocol members gives incompatible
//!    types).
//!
//! ```python
//! class Proto(Protocol):
//!     def meth(self) -> int: ...
//!
//! class Concrete:
//!     def meth(self) -> int: return 42
//!
//! def fun(cls: type[Proto]) -> int:
//!     return cls().meth()
//!
//! fun(Proto)      # E0146 — Protocol class itself passed to type[Proto]
//! fun(Concrete)   # OK
//!
//! var: type[Proto]
//! var = Proto     # E0146 — Protocol class assigned to type[Proto]
//! var = Concrete  # OK
//!
//! pa1: ProtoA1 = ConcreteA  # E0146 — class object can't satisfy instance protocol
//! pa2: ProtoA2 = ConcreteA  # OK    — protocol uses _self/self pattern
//! ```

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::rules::shared::{ann_str, expr_name};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0146",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0146",
};

/// Emits BSK-E0146 for Protocol class object violations.
pub(crate) struct ProtocolClassObjectViolation;

impl Rule for ProtocolClassObjectViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        let ctx = ModuleCtx::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
        check_stmts_with_funcs(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level context collected from the AST
// ---------------------------------------------------------------------------

/// Describes how a protocol method/attribute is accessible on a class object.
#[derive(Debug, Clone)]
enum ProtocolMember {
    /// A regular instance method — first param is `self`.
    InstanceMethod,
    /// A method designed for class-object use — first param is NOT `self`
    /// (e.g. `_self`), indicating it accounts for the class being the receiver.
    ClassObjectMethod,
    /// A `@property`-decorated attribute.
    Property,
    /// A `ClassVar[T]` annotated attribute.
    ClassVar,
    /// A plain instance attribute (no `ClassVar`), with its attribute name.
    InstanceAttr { name: String },
}

/// Summary of a Protocol class.
#[derive(Debug, Clone)]
struct ProtocolInfo {
    name: String,
    members: Vec<ProtocolMember>,
}

/// Summary of a concrete (non-Protocol) class.
#[derive(Debug, Clone)]
struct ConcreteClassInfo {
    name: String,
    /// Names of class-variable attributes (`ClassVar` annotations).
    class_vars: Vec<String>,
    /// Whether the class has a custom metaclass that may provide instance attrs.
    has_custom_metaclass: bool,
}

struct ModuleCtx {
    protocols: Vec<ProtocolInfo>,
    concrete_classes: Vec<ConcreteClassInfo>,
    func_sigs: Vec<FuncSig>,
}

impl ModuleCtx {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut protocols = Vec::new();
        let mut concrete_classes = Vec::new();
        let mut func_sigs = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    if is_protocol_class(cls) {
                        protocols.push(extract_protocol_info(cls));
                    } else {
                        concrete_classes.push(extract_concrete_info(cls));
                    }
                }
                Stmt::FunctionDef(func) => {
                    func_sigs.push(extract_func_sig(func));
                }
                _ => {}
            }
        }
        Self {
            protocols,
            concrete_classes,
            func_sigs,
        }
    }

    fn find_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.iter().find(|p| p.name == name)
    }

    fn is_protocol(&self, name: &str) -> bool {
        self.protocols.iter().any(|p| p.name == name)
    }

    fn find_concrete(&self, name: &str) -> Option<&ConcreteClassInfo> {
        self.concrete_classes.iter().find(|c| c.name == name)
    }
}

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

fn is_protocol_class(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|arg| match arg {
            Expr::Name(n) => n.id.as_str() == "Protocol",
            Expr::Subscript(sub) => {
                matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Protocol")
            }
            _ => false,
        })
    })
}

fn extract_protocol_info(cls: &ast::StmtClassDef) -> ProtocolInfo {
    let mut members = Vec::new();
    for stmt in &cls.body {
        match stmt {
            Stmt::FunctionDef(func) => {
                let is_property = func.decorator_list.iter().any(|dec| {
                    matches!(&dec.expression, Expr::Name(n) if n.id.as_str() == "property")
                        || matches!(&dec.expression, Expr::Attribute(a)
                            if a.attr.as_str() == "getter" || a.attr.as_str() == "setter")
                });
                if is_property {
                    members.push(ProtocolMember::Property);
                } else {
                    // Check whether the first parameter is named `self`.
                    let first_param_name = func
                        .parameters
                        .posonlyargs
                        .first()
                        .or_else(|| func.parameters.args.first())
                        .map(|p| p.parameter.name.as_str());

                    match first_param_name {
                        Some("self") => {
                            members.push(ProtocolMember::InstanceMethod);
                        }
                        _ => {
                            // First param absent or not named `self` — designed for class-object use.
                            members.push(ProtocolMember::ClassObjectMethod);
                        }
                    }
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Some(attr_name) = expr_name(&ann.target) {
                    if extract_classvar_inner(&ann.annotation).is_some() {
                        members.push(ProtocolMember::ClassVar);
                    } else {
                        members.push(ProtocolMember::InstanceAttr {
                            name: attr_name.to_owned(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    ProtocolInfo {
        name: cls.name.to_string(),
        members,
    }
}

fn extract_concrete_info(cls: &ast::StmtClassDef) -> ConcreteClassInfo {
    let mut class_vars = Vec::new();

    // Detect custom metaclass (anything other than bare `type`).
    let has_custom_metaclass = cls.arguments.as_ref().is_some_and(|args| {
        args.keywords.iter().any(|kw| {
            kw.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass")
                && !matches!(&kw.value, Expr::Name(n) if n.id.as_str() == "type")
        })
    });

    for stmt in &cls.body {
        if let Stmt::AnnAssign(ann) = stmt {
            if let Some(attr_name) = expr_name(&ann.target) {
                if extract_classvar_inner(&ann.annotation).is_some() {
                    class_vars.push(attr_name.to_owned());
                }
            }
        }
    }

    ConcreteClassInfo {
        name: cls.name.to_string(),
        class_vars,
        has_custom_metaclass,
    }
}

/// Extract the inner type string from `ClassVar[T]`.
fn extract_classvar_inner(expr: &Expr) -> Option<String> {
    if let Expr::Subscript(sub) = expr {
        if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "ClassVar") {
            return Some(ann_str(&sub.slice));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Function signature info (for call-site checking)
// ---------------------------------------------------------------------------

struct FuncSig {
    name: String,
    /// Annotation strings for each positional parameter (in order).
    params: Vec<Option<String>>,
}

fn extract_func_sig(func: &ast::StmtFunctionDef) -> FuncSig {
    let mut params = Vec::new();
    for p in &func.parameters.posonlyargs {
        params.push(p.parameter.annotation.as_ref().map(|a| ann_str(a)));
    }
    for p in &func.parameters.args {
        params.push(p.parameter.annotation.as_ref().map(|a| ann_str(a)));
    }
    FuncSig {
        name: func.name.to_string(),
        params,
    }
}

// ---------------------------------------------------------------------------
// Statement traversal
// ---------------------------------------------------------------------------

/// Check annotated assignments and plain assignments (for `var: type[Proto]` then `var = Proto`).
fn check_stmts(stmts: &[Stmt], ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    // Track variables annotated as `type[Proto]` so we can check plain assignments.
    let mut type_proto_vars: Vec<(String, String)> = Vec::new();

    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                let span = Span {
                    start: ann.range().start().to_u32(),
                    end: ann.range().end().to_u32(),
                };

                // Track `var: type[Proto]` declarations.
                if let Some(proto_name) = extract_type_proto_name(&ann.annotation) {
                    if ctx.is_protocol(&proto_name) {
                        if let Some(var_name) = expr_name(&ann.target) {
                            type_proto_vars.push((var_name.to_owned(), proto_name.clone()));
                        }
                    }
                }

                if let Some(value) = &ann.value {
                    check_ann_assign(&ann.annotation, value, ctx, path, diag, span);
                }
            }
            Stmt::Assign(assign)
                // Check plain assignment `var = Proto` where `var: type[Proto]` was declared.
                if assign.targets.len() == 1 => {
                    if let Some(target_name) = assign.targets.first().and_then(expr_name) {
                        if let Some((_, proto_name)) =
                            type_proto_vars.iter().find(|(n, _)| n == target_name)
                        {
                            if let Some(value_name) = expr_name(&assign.value) {
                                if value_name == proto_name.as_str() && ctx.is_protocol(proto_name)
                                {
                                    let span = Span {
                                        start: assign.range().start().to_u32(),
                                        end: assign.range().end().to_u32(),
                                    };
                                    diag.push(make_type_proto_diag(proto_name, path, span));
                                }
                            }
                        }
                    }
                }
            _ => {}
        }
    }
}

/// Check call expressions for `fun(Proto)` where the param is `type[Proto]`.
fn check_stmts_with_funcs(stmts: &[Stmt], ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        if let Stmt::Expr(expr_stmt) = stmt {
            if let Expr::Call(call) = &*expr_stmt.value {
                let span = Span {
                    start: expr_stmt.range().start().to_u32(),
                    end: expr_stmt.range().end().to_u32(),
                };
                check_call_with_sigs(call, &ctx.func_sigs, ctx, path, diag, span);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Annotated assignment check
// ---------------------------------------------------------------------------

fn check_ann_assign(
    annotation: &Expr,
    value: &Expr,
    ctx: &ModuleCtx,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let Some(value_name) = expr_name(value) else {
        return;
    };

    // Case 1: `var: type[Proto] = Proto` — Protocol class used where `type[Proto]` expected.
    if let Some(proto_name) = extract_type_proto_name(annotation) {
        if value_name == proto_name && ctx.is_protocol(&proto_name) {
            diag.push(make_type_proto_diag(&proto_name, path, span));
            return;
        }
    }

    // Case 2: `var: ProtoName = ConcreteClass` — class object used where instance expected.
    let ann_name = ann_str(annotation);
    if let Some(protocol) = ctx.find_protocol(&ann_name) {
        if let Some(concrete) = ctx.find_concrete(value_name) {
            if !class_satisfies_protocol_as_object(concrete, protocol) {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class `{}` cannot be used as an instance of protocol `{}`: \
                         class-level access to protocol members is incompatible",
                        concrete.name, protocol.name
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Pass an instance of a class implementing `{}` instead of the class itself",
                        protocol.name
                    )),
                    note: Some(
                        "A class object satisfies a protocol only if all protocol members \
                         are accessible on the class with compatible types"
                            .to_owned(),
                    ),
                    provenance: None,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Call-site check
// ---------------------------------------------------------------------------

fn check_call_with_sigs(
    call: &ast::ExprCall,
    func_sigs: &[FuncSig],
    ctx: &ModuleCtx,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let Some(func_name) = expr_name(&call.func) else {
        return;
    };
    let Some(sig) = func_sigs.iter().find(|f| f.name == func_name) else {
        return;
    };

    for (arg_idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(arg_name) = expr_name(arg) else {
            continue;
        };
        let Some(param_ann) = sig.params.get(arg_idx).and_then(|a| a.as_deref()) else {
            continue;
        };
        if let Some(proto_name) = extract_type_proto_from_str(param_ann) {
            if arg_name == proto_name && ctx.is_protocol(proto_name) {
                diag.push(make_type_proto_diag(proto_name, path, span));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol structural compatibility for class objects
// ---------------------------------------------------------------------------

/// Returns `true` if the class, used as an object (not an instance), structurally
/// satisfies the given protocol.
///
/// Rules:
/// - Regular instance methods (`self` as first param): incompatible — class-level
///   access gives an unbound function requiring `self`.
/// - Class-object methods (first param not `self`, e.g. `_self`): compatible.
/// - `@property` members: incompatible — class access gives a descriptor, not the value.
/// - `ClassVar` protocol attrs: incompatible — would require a metaclass `ClassVar`.
/// - Instance attrs: compatible only if the class has a matching `ClassVar` or a
///   custom metaclass that can provide the attribute on the class object.
fn class_satisfies_protocol_as_object(class: &ConcreteClassInfo, protocol: &ProtocolInfo) -> bool {
    for member in &protocol.members {
        match member {
            ProtocolMember::InstanceMethod
            | ProtocolMember::Property
            | ProtocolMember::ClassVar => return false,
            ProtocolMember::ClassObjectMethod => {
                // Compatible — no action needed.
            }
            ProtocolMember::InstanceAttr { name } => {
                let has_classvar = class.class_vars.iter().any(|cv| cv == name);
                if has_classvar || class.has_custom_metaclass {
                    continue;
                }
                return false;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// `type[Proto]` annotation helpers
// ---------------------------------------------------------------------------

/// Extract the inner name from a `type[X]` annotation expression.
fn extract_type_proto_name(expr: &Expr) -> Option<String> {
    if let Expr::Subscript(sub) = expr {
        if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "type") {
            return Some(ann_str(&sub.slice));
        }
    }
    None
}

/// Extract the inner name from a `type[X]` annotation string.
fn extract_type_proto_from_str(s: &str) -> Option<&str> {
    let inner = s.strip_prefix("type[")?;
    inner.strip_suffix(']').map(str::trim)
}

fn make_type_proto_diag(proto_name: &str, path: &str, span: Span) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Protocol class `{proto_name}` cannot be used where `type[{proto_name}]` is expected; \
             only concrete (non-protocol) subtypes are accepted"
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Pass a concrete class that implements `{proto_name}` instead of the Protocol class itself"
        )),
        note: Some(
            "Variables and parameters annotated with `type[Proto]` accept only \
             concrete (non-protocol) subtypes of Proto"
                .to_owned(),
        ),
        provenance: None,
    }
}
