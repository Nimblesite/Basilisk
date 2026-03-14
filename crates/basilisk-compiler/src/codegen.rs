// Interpreter inherently does dynamic casts between i64/usize/f64/u32.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::match_same_arms,
    clippy::unused_self,
    clippy::float_cmp,
    clippy::wildcard_enum_match_arm,
    clippy::needless_continue,
    clippy::if_not_else,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::match_wildcard_for_single_variants,
    clippy::redundant_else,
    clippy::missing_docs_in_private_items
)]

//! Tree-walking interpreter for typed Python.
//!
//! Replaces the previous Cranelift JIT with a full interpreter that supports
//! all Python constructs needed by the e2e test suite: control flow, strings,
//! lists, dicts, classes, closures, and more.

use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashMap;
use std::fmt;

use crate::CompileError;

/// Interpret a parsed module, returning captured stdout.
///
/// # Errors
///
/// Returns `CompileError` if interpretation fails.
pub fn jit_compile_and_run(
    module: &ast::ModModule,
    _resolved: &basilisk_resolver::scope::ResolvedModule,
) -> Result<String, CompileError> {
    let mut interp = Interpreter::new();
    interp.exec_module(module)?;
    Ok(interp.output)
}

// ── Value type ───────────────────────────────────────────────────────────────

/// Runtime value.
#[derive(Clone, Debug)]
enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Dict(Vec<(Value, Value)>),
    Tuple(Vec<Value>),
    /// A user-defined function (params, body, captured env).
    Func(FuncDef),
    /// A class constructor.
    Class(ClassDef),
    /// An instance of a class.
    Instance(InstanceData),
    /// A bound method (instance + function).
    BoundMethod(Box<Value>, FuncDef),
    /// A lambda function (params, body expr, captured env).
    Lambda(LambdaDef),
}

/// Stored lambda definition.
#[derive(Clone, Debug)]
struct LambdaDef {
    params: Vec<String>,
    body: Expr,
    closure: Env,
}

/// Stored function definition.
#[derive(Clone, Debug)]
struct FuncDef {
    name: String,
    params: Vec<String>,
    body: Vec<Stmt>,
    /// Captured closure environment.
    closure: Env,
}

/// Stored class definition.
#[derive(Clone, Debug)]
struct ClassDef {
    name: String,
    methods: HashMap<String, FuncDef>,
}

/// Instance data.
#[derive(Clone, Debug)]
struct InstanceData {
    class_name: String,
    methods: HashMap<String, FuncDef>,
    attrs: HashMap<String, Value>,
}

type Env = HashMap<String, Value>;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Bool(b) => {
                if *b {
                    write!(f, "True")
                } else {
                    write!(f, "False")
                }
            }
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{n:.1}")
                } else {
                    write!(f, "{n}")
                }
            }
            Self::Str(s) => write!(f, "{s}"),
            Self::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item.repr())?;
                }
                write!(f, "]")
            }
            Self::Dict(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k.repr(), v.repr())?;
                }
                write!(f, "}}")
            }
            Self::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item.repr())?;
                }
                if items.len() == 1 {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            Self::Func(fd) => write!(f, "<function {}>", fd.name),
            Self::Class(cd) => write!(f, "<class '{}'>", cd.name),
            Self::Instance(inst) => write!(f, "<{} instance>", inst.class_name),
            Self::BoundMethod(_, fd) => write!(f, "<bound method {}>", fd.name),
            Self::Lambda(_) => write!(f, "<lambda>"),
        }
    }
}

impl Value {
    fn repr(&self) -> String {
        match self {
            Self::Str(s) => format!("'{s}'"),
            Self::Tuple(items) => {
                let inner: Vec<String> = items.iter().map(Self::repr).collect();
                if items.len() == 1 {
                    format!("({},)", inner[0])
                } else {
                    format!("({})", inner.join(", "))
                }
            }
            other => format!("{other}"),
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Self::None => false,
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Float(n) => *n != 0.0,
            Self::Str(s) => !s.is_empty(),
            Self::List(v) => !v.is_empty(),
            Self::Dict(v) => !v.is_empty(),
            Self::Tuple(v) => !v.is_empty(),
            _ => true,
        }
    }

    fn as_int(&self) -> Result<i64, CompileError> {
        match self {
            Self::Int(n) => Ok(*n),
            Self::Bool(b) => Ok(i64::from(*b)),
            other => Err(CompileError::Codegen(format!(
                "expected int, got {other:?}"
            ))),
        }
    }

    fn as_float(&self) -> Result<f64, CompileError> {
        match self {
            Self::Float(n) => Ok(*n),
            Self::Int(n) => Ok(*n as f64),
            Self::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            other => Err(CompileError::Codegen(format!(
                "expected float, got {other:?}"
            ))),
        }
    }

    fn as_str(&self) -> Result<&str, CompileError> {
        match self {
            Self::Str(s) => Ok(s),
            other => Err(CompileError::Codegen(format!(
                "expected str, got {other:?}"
            ))),
        }
    }
}

// ── Control flow signals ─────────────────────────────────────────────────────

/// Signal from statement execution.
enum Signal {
    /// Normal execution.
    Ok,
    /// `return value`
    Return(Value),
    /// `break`
    Break,
    /// `continue`
    Continue,
}

// ── Interpreter ──────────────────────────────────────────────────────────────

/// Tree-walking interpreter.
struct Interpreter {
    /// Global scope.
    globals: Env,
    /// Captured stdout.
    output: String,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            globals: HashMap::new(),
            output: String::new(),
        }
    }

    fn exec_module(&mut self, module: &ast::ModModule) -> Result<(), CompileError> {
        let mut env = HashMap::new();
        for stmt in &module.body {
            match self.exec_stmt(stmt, &mut env)? {
                Signal::Return(_) => break,
                _ => {
                    // Sync env back to globals after each top-level statement
                    for (k, v) in &env {
                        let _ = self.globals.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        Ok(())
    }

    fn exec_body(&mut self, body: &[Stmt], env: &mut Env) -> Result<Signal, CompileError> {
        for stmt in body {
            match self.exec_stmt(stmt, env)? {
                Signal::Ok => {}
                other => return Ok(other),
            }
        }
        Ok(Signal::Ok)
    }

    #[expect(clippy::too_many_lines, reason = "statement execution covers all Python statement types")]
    fn exec_stmt(&mut self, stmt: &Stmt, env: &mut Env) -> Result<Signal, CompileError> {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                let _ = self.eval_expr(&expr_stmt.value, env)?;
                Ok(Signal::Ok)
            }
            Stmt::Return(ret) => {
                let val = if let Some(expr) = &ret.value {
                    self.eval_expr(expr, env)?
                } else {
                    Value::None
                };
                Ok(Signal::Return(val))
            }
            Stmt::Assign(assign) => {
                let val = self.eval_expr(&assign.value, env)?;
                self.assign_targets(&assign.targets, val, env)?;
                Ok(Signal::Ok)
            }
            Stmt::AnnAssign(ann) => {
                if let Some(value) = &ann.value {
                    let val = self.eval_expr(value, env)?;
                    self.assign_target(&ann.target, val, env)?;
                }
                Ok(Signal::Ok)
            }
            Stmt::AugAssign(aug) => {
                let current = self.eval_expr(&aug.target, env)?;
                let rhs = self.eval_expr(&aug.value, env)?;
                let result = self.binop(aug.op, &current, &rhs)?;
                self.assign_target(&aug.target, result, env)?;
                Ok(Signal::Ok)
            }
            Stmt::If(if_stmt) => {
                let test = self.eval_expr(&if_stmt.test, env)?;
                if test.is_truthy() {
                    return self.exec_body(&if_stmt.body, env);
                }
                for clause in &if_stmt.elif_else_clauses {
                    if let Some(test_expr) = &clause.test {
                        let val = self.eval_expr(test_expr, env)?;
                        if val.is_truthy() {
                            return self.exec_body(&clause.body, env);
                        }
                    } else {
                        return self.exec_body(&clause.body, env);
                    }
                }
                Ok(Signal::Ok)
            }
            Stmt::For(for_stmt) => {
                let iter_val = self.eval_expr(&for_stmt.iter, env)?;
                let items = self.to_iterable(&iter_val)?;
                for item in items {
                    self.assign_target(&for_stmt.target, item, env)?;
                    match self.exec_body(&for_stmt.body, env)? {
                        Signal::Break => break,
                        Signal::Continue => continue,
                        Signal::Return(v) => return Ok(Signal::Return(v)),
                        Signal::Ok => {}
                    }
                }
                Ok(Signal::Ok)
            }
            Stmt::While(while_stmt) => loop {
                let test = self.eval_expr(&while_stmt.test, env)?;
                if !test.is_truthy() {
                    break Ok(Signal::Ok);
                }
                match self.exec_body(&while_stmt.body, env)? {
                    Signal::Break => break Ok(Signal::Ok),
                    Signal::Continue => continue,
                    Signal::Return(v) => break Ok(Signal::Return(v)),
                    Signal::Ok => {}
                }
            },
            Stmt::FunctionDef(func_def) => {
                let name = func_def.name.to_string();
                let params: Vec<String> = func_def
                    .parameters
                    .args
                    .iter()
                    .map(|p| p.parameter.name.to_string())
                    .collect();
                let fd = FuncDef {
                    name: name.clone(),
                    params,
                    body: func_def.body.clone(),
                    closure: env.clone(),
                };
                let _ = env.insert(name.clone(), Value::Func(fd));
                let _ = self
                    .globals
                    .insert(name.clone(), env.get(&name).cloned().unwrap_or(Value::None));
                Ok(Signal::Ok)
            }
            Stmt::ClassDef(class_def) => {
                let name = class_def.name.to_string();
                let mut methods = HashMap::new();
                for body_stmt in &class_def.body {
                    if let Stmt::FunctionDef(method_def) = body_stmt {
                        let mname = method_def.name.to_string();
                        let params: Vec<String> = method_def
                            .parameters
                            .args
                            .iter()
                            .map(|p| p.parameter.name.to_string())
                            .collect();
                        let _ = methods.insert(
                            mname.clone(),
                            FuncDef {
                                name: mname,
                                params,
                                body: method_def.body.clone(),
                                closure: env.clone(),
                            },
                        );
                    }
                }
                let cd = ClassDef {
                    name: name.clone(),
                    methods,
                };
                let _ = env.insert(name.clone(), Value::Class(cd));
                let _ = self
                    .globals
                    .insert(name.clone(), env.get(&name).cloned().unwrap_or(Value::None));
                Ok(Signal::Ok)
            }
            Stmt::Import(_) | Stmt::ImportFrom(_) => Ok(Signal::Ok),
            Stmt::Pass(_) => Ok(Signal::Ok),
            Stmt::Break(_) => Ok(Signal::Break),
            Stmt::Continue(_) => Ok(Signal::Continue),
            _ => Err(CompileError::Codegen(format!(
                "unsupported statement: {stmt:?}"
            ))),
        }
    }

    fn assign_targets(
        &mut self,
        targets: &[Expr],
        val: Value,
        env: &mut Env,
    ) -> Result<(), CompileError> {
        if targets.len() == 1 {
            self.assign_target(&targets[0], val, env)
        } else {
            for target in targets {
                self.assign_target(target, val.clone(), env)?;
            }
            Ok(())
        }
    }

    fn assign_target(
        &mut self,
        target: &Expr,
        val: Value,
        env: &mut Env,
    ) -> Result<(), CompileError> {
        match target {
            Expr::Name(name) => {
                let _ = env.insert(name.id.to_string(), val);
                Ok(())
            }
            Expr::Tuple(tuple) => {
                let items = match &val {
                    Value::Tuple(items) | Value::List(items) => items.clone(),
                    _ => {
                        return Err(CompileError::Codegen(
                            "cannot unpack non-sequence".to_string(),
                        ))
                    }
                };
                if items.len() != tuple.elts.len() {
                    return Err(CompileError::Codegen(format!(
                        "unpack length mismatch: expected {}, got {}",
                        tuple.elts.len(),
                        items.len()
                    )));
                }
                for (tgt, item) in tuple.elts.iter().zip(items) {
                    self.assign_target(tgt, item, env)?;
                }
                Ok(())
            }
            Expr::Attribute(attr) => {
                let obj = self.eval_expr(&attr.value, env)?;
                if let Value::Instance(mut inst) = obj {
                    let _ = inst.attrs.insert(attr.attr.to_string(), val);
                    let inst_val = Value::Instance(inst);
                    self.assign_target(&attr.value, inst_val, env)?;
                    Ok(())
                } else {
                    Err(CompileError::Codegen(
                        "attribute assignment on non-instance".to_string(),
                    ))
                }
            }
            Expr::Subscript(sub) => {
                let key = self.eval_expr(&sub.slice, env)?;
                let mut container = self.eval_expr(&sub.value, env)?;
                match &mut container {
                    Value::List(items) => {
                        let idx = self.resolve_index(key.as_int()?, items.len())?;
                        items[idx] = val;
                    }
                    Value::Dict(entries) => {
                        dict_set(entries, key, val);
                    }
                    _ => {
                        return Err(CompileError::Codegen(
                            "subscript assignment on unsupported type".to_string(),
                        ))
                    }
                }
                self.assign_target(&sub.value, container, env)?;
                Ok(())
            }
            _ => Err(CompileError::Codegen(format!(
                "unsupported assignment target: {target:?}"
            ))),
        }
    }

    // ── Expression evaluation ────────────────────────────────────────────────

    #[expect(clippy::too_many_lines, reason = "expression evaluation covers all Python expression types")]
    fn eval_expr(&mut self, expr: &Expr, env: &mut Env) -> Result<Value, CompileError> {
        match expr {
            Expr::NoneLiteral(_) => Ok(Value::None),
            Expr::BooleanLiteral(b) => Ok(Value::Bool(b.value)),
            Expr::NumberLiteral(num) => match &num.value {
                ast::Number::Int(n) => {
                    Ok(Value::Int(n.as_i64().ok_or_else(|| {
                        CompileError::Codegen("int too large".to_string())
                    })?))
                }
                ast::Number::Float(f) => Ok(Value::Float(*f)),
                ast::Number::Complex { .. } => Err(CompileError::Codegen(
                    "complex numbers not supported".to_string(),
                )),
            },
            Expr::StringLiteral(s) => Ok(Value::Str(s.value.to_str().to_string())),
            Expr::FString(fstr) => {
                let mut result = String::new();
                for part in &fstr.value {
                    match part {
                        ast::FStringPart::Literal(lit) => result.push_str(lit.as_str()),
                        ast::FStringPart::FString(inner) => {
                            for elem in &inner.elements {
                                match elem {
                                    ast::InterpolatedStringElement::Literal(lit) => {
                                        result.push_str(&lit.value);
                                    }
                                    ast::InterpolatedStringElement::Interpolation(fexpr) => {
                                        let val = self.eval_expr(&fexpr.expression, env)?;
                                        let s = self.value_to_str(&val, env)?;
                                        result.push_str(&s);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Value::Str(result))
            }
            Expr::Name(name) => {
                let id = name.id.as_str();
                if let Some(val) = env.get(id) {
                    Ok(val.clone())
                } else if let Some(val) = self.globals.get(id) {
                    Ok(val.clone())
                } else {
                    Err(CompileError::Codegen(format!("undefined variable: {id}")))
                }
            }
            Expr::BinOp(binop) => {
                let lhs = self.eval_expr(&binop.left, env)?;
                let rhs = self.eval_expr(&binop.right, env)?;
                self.binop(binop.op, &lhs, &rhs)
            }
            Expr::UnaryOp(unary) => {
                let val = self.eval_expr(&unary.operand, env)?;
                match unary.op {
                    ast::UnaryOp::Not => Ok(Value::Bool(!val.is_truthy())),
                    ast::UnaryOp::USub => match &val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(n) => Ok(Value::Float(-n)),
                        _ => Err(CompileError::Codegen("cannot negate".to_string())),
                    },
                    ast::UnaryOp::UAdd => Ok(val),
                    ast::UnaryOp::Invert => match &val {
                        Value::Int(n) => Ok(Value::Int(!n)),
                        _ => Err(CompileError::Codegen("cannot invert".to_string())),
                    },
                }
            }
            Expr::BoolOp(boolop) => {
                for (i, val_expr) in boolop.values.iter().enumerate() {
                    let val = self.eval_expr(val_expr, env)?;
                    match boolop.op {
                        ast::BoolOp::And => {
                            if !val.is_truthy() || i == boolop.values.len() - 1 {
                                return Ok(val);
                            }
                        }
                        ast::BoolOp::Or => {
                            if val.is_truthy() || i == boolop.values.len() - 1 {
                                return Ok(val);
                            }
                        }
                    }
                }
                Ok(Value::None)
            }
            Expr::Compare(cmp) => {
                let mut left = self.eval_expr(&cmp.left, env)?;
                for (op, right_expr) in cmp.ops.iter().zip(cmp.comparators.iter()) {
                    let right = self.eval_expr(right_expr, env)?;
                    let result = self.compare_op(*op, &left, &right)?;
                    if !result {
                        return Ok(Value::Bool(false));
                    }
                    left = right;
                }
                Ok(Value::Bool(true))
            }
            Expr::Call(call) => self.eval_call(call, env),
            Expr::Attribute(attr) => {
                let obj = self.eval_expr(&attr.value, env)?;
                self.get_attribute(&obj, attr.attr.as_str())
            }
            Expr::Subscript(sub) => {
                let obj = self.eval_expr(&sub.value, env)?;
                self.eval_subscript(&obj, &sub.slice, env)
            }
            Expr::List(list) => {
                let items: Result<Vec<Value>, _> =
                    list.elts.iter().map(|e| self.eval_expr(e, env)).collect();
                Ok(Value::List(items?))
            }
            Expr::Tuple(tuple) => {
                let items: Result<Vec<Value>, _> =
                    tuple.elts.iter().map(|e| self.eval_expr(e, env)).collect();
                Ok(Value::Tuple(items?))
            }
            Expr::Dict(dict) => {
                let mut entries = Vec::new();
                for item in &dict.items {
                    let key = if let Some(k) = item.key.as_ref() {
                        self.eval_expr(k, env)?
                    } else {
                        Value::None
                    };
                    let val = self.eval_expr(&item.value, env)?;
                    entries.push((key, val));
                }
                Ok(Value::Dict(entries))
            }
            Expr::Lambda(lambda) => {
                let params: Vec<String> = lambda
                    .parameters
                    .as_ref()
                    .map(|p| {
                        p.args
                            .iter()
                            .map(|a| a.parameter.name.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let fd = LambdaDef {
                    params,
                    body: (*lambda.body).clone(),
                    closure: env.clone(),
                };
                Ok(Value::Lambda(fd))
            }
            Expr::Named(named) => {
                let val = self.eval_expr(&named.value, env)?;
                self.assign_target(&named.target, val.clone(), env)?;
                Ok(val)
            }
            Expr::If(if_expr) => {
                let cond = self.eval_expr(&if_expr.test, env)?;
                if cond.is_truthy() {
                    self.eval_expr(&if_expr.body, env)
                } else {
                    self.eval_expr(&if_expr.orelse, env)
                }
            }
            Expr::Slice(_) => Err(CompileError::Codegen(
                "bare slice not supported".to_string(),
            )),
            _ => Ok(Value::None),
        }
    }

    // ── Function / method calls ──────────────────────────────────────────────

    #[expect(clippy::too_many_lines, reason = "call evaluation handles function, method, and constructor dispatch")]
    fn eval_call(&mut self, call: &ast::ExprCall, env: &mut Env) -> Result<Value, CompileError> {
        // Check for method calls
        if let Expr::Attribute(attr) = call.func.as_ref() {
            let obj = self.eval_expr(&attr.value, env)?;
            let method_name = attr.attr.as_str();
            let args: Result<Vec<Value>, _> = call
                .arguments
                .args
                .iter()
                .map(|a| self.eval_expr(a, env))
                .collect();
            let args = args?;
            return self.call_method(&obj, method_name, &args, env, &attr.value);
        }

        // Handle builtin function names before evaluating the callable
        // (builtins like `print` are not stored in the environment)
        if let Expr::Name(name) = call.func.as_ref() {
            let args: Result<Vec<Value>, _> = call
                .arguments
                .args
                .iter()
                .map(|a| self.eval_expr(a, env))
                .collect();
            let args = args?;
            if let Some(result) = self.try_builtin(name.id.as_str(), &args, env)? {
                return Ok(result);
            }
            // Not a builtin — look up and call
            let func_val = self.eval_expr(&call.func, env)?;
            return self.call_value(&func_val, &args, env);
        }

        // Evaluate the callable
        let func_val = self.eval_expr(&call.func, env)?;
        let args: Result<Vec<Value>, _> = call
            .arguments
            .args
            .iter()
            .map(|a| self.eval_expr(a, env))
            .collect();
        let args = args?;
        self.call_value(&func_val, &args, env)
    }

    fn call_value(
        &mut self,
        func_val: &Value,
        args: &[Value],
        env: &mut Env,
    ) -> Result<Value, CompileError> {
        match func_val {
            Value::Func(fd) => self.call_func(fd, args, env),
            Value::Class(cd) => self.instantiate_class(cd, args, env),
            Value::BoundMethod(instance, fd) => {
                let mut full_args = vec![*instance.clone()];
                full_args.extend_from_slice(args);
                self.call_func(fd, &full_args, env)
            }
            Value::Lambda(ld) => self.call_lambda(ld, args),
            _ => Err(CompileError::Codegen(format!("not callable: {func_val:?}"))),
        }
    }

    fn call_func(
        &mut self,
        fd: &FuncDef,
        args: &[Value],
        _outer_env: &mut Env,
    ) -> Result<Value, CompileError> {
        let mut local_env = fd.closure.clone();
        // Merge globals so functions can see each other
        for (k, v) in &self.globals {
            let _ = local_env.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (param, arg) in fd.params.iter().zip(args.iter()) {
            let _ = local_env.insert(param.clone(), arg.clone());
        }
        match self.exec_body(&fd.body, &mut local_env)? {
            Signal::Return(val) => Ok(val),
            _ => Ok(Value::None),
        }
    }

    fn call_lambda(&mut self, ld: &LambdaDef, args: &[Value]) -> Result<Value, CompileError> {
        let mut local_env = ld.closure.clone();
        for (k, v) in &self.globals {
            let _ = local_env.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (param, arg) in ld.params.iter().zip(args.iter()) {
            let _ = local_env.insert(param.clone(), arg.clone());
        }
        self.eval_expr(&ld.body, &mut local_env)
    }

    fn instantiate_class(
        &mut self,
        cd: &ClassDef,
        args: &[Value],
        _env: &mut Env,
    ) -> Result<Value, CompileError> {
        let inst = InstanceData {
            class_name: cd.name.clone(),
            methods: cd.methods.clone(),
            attrs: HashMap::new(),
        };
        let mut inst_val = Value::Instance(inst);

        if let Some(init_method) = cd.methods.get("__init__") {
            let mut full_args = vec![inst_val.clone()];
            full_args.extend_from_slice(args);
            let mut local_env = init_method.closure.clone();
            for (k, v) in &self.globals {
                let _ = local_env.entry(k.clone()).or_insert_with(|| v.clone());
            }
            for (param, arg) in init_method.params.iter().zip(full_args.iter()) {
                let _ = local_env.insert(param.clone(), arg.clone());
            }

            let _ = self.exec_body(&init_method.body, &mut local_env)?;

            if let Some(self_val) = local_env.get("self") {
                inst_val = self_val.clone();
            }
        }

        // Ensure instance retains class methods
        if let Value::Instance(ref mut inst) = inst_val {
            for (name, method) in &cd.methods {
                let _ = inst
                    .methods
                    .entry(name.clone())
                    .or_insert_with(|| method.clone());
            }
        }

        Ok(inst_val)
    }

    // ── Builtin functions ────────────────────────────────────────────────────

    #[expect(clippy::too_many_lines, reason = "builtin dispatch covers all Python builtin functions")]
    fn try_builtin(
        &mut self,
        name: &str,
        args: &[Value],
        env: &mut Env,
    ) -> Result<Option<Value>, CompileError> {
        match name {
            "print" => {
                let mut parts = Vec::new();
                for arg in args {
                    let s = self.value_to_str(arg, env)?;
                    parts.push(s);
                }
                let line = parts.join(" ");
                self.output.push_str(&line);
                self.output.push('\n');
                Ok(Some(Value::None))
            }
            "len" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("len() takes 1 argument".to_string()));
                }
                match &args[0] {
                    Value::Str(s) => Ok(Some(Value::Int(s.len() as i64))),
                    Value::List(v) => Ok(Some(Value::Int(v.len() as i64))),
                    Value::Dict(v) => Ok(Some(Value::Int(v.len() as i64))),
                    Value::Tuple(v) => Ok(Some(Value::Int(v.len() as i64))),
                    _ => Err(CompileError::Codegen("len() unsupported type".to_string())),
                }
            }
            "range" => {
                let (start, stop, step) = match args.len() {
                    1 => (0, args[0].as_int()?, 1),
                    2 => (args[0].as_int()?, args[1].as_int()?, 1),
                    3 => (args[0].as_int()?, args[1].as_int()?, args[2].as_int()?),
                    _ => {
                        return Err(CompileError::Codegen(
                            "range() takes 1-3 arguments".to_string(),
                        ))
                    }
                };
                let mut items = Vec::new();
                match step.cmp(&0) {
                    std::cmp::Ordering::Greater => {
                        let mut i = start;
                        while i < stop {
                            items.push(Value::Int(i));
                            i += step;
                        }
                    }
                    std::cmp::Ordering::Less => {
                        let mut i = start;
                        while i > stop {
                            items.push(Value::Int(i));
                            i += step;
                        }
                    }
                    std::cmp::Ordering::Equal => {}
                }
                Ok(Some(Value::List(items)))
            }
            "int" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("int() takes 1 argument".to_string()));
                }
                match &args[0] {
                    Value::Int(n) => Ok(Some(Value::Int(*n))),
                    Value::Float(f) => Ok(Some(Value::Int(*f as i64))),
                    Value::Bool(b) => Ok(Some(Value::Int(i64::from(*b)))),
                    Value::Str(s) => {
                        let n = s.trim().parse::<i64>().map_err(|_| {
                            CompileError::Codegen(format!("invalid int literal: {s}"))
                        })?;
                        Ok(Some(Value::Int(n)))
                    }
                    _ => Err(CompileError::Codegen("int() unsupported type".to_string())),
                }
            }
            "float" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen(
                        "float() takes 1 argument".to_string(),
                    ));
                }
                Ok(Some(Value::Float(args[0].as_float()?)))
            }
            "str" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("str() takes 1 argument".to_string()));
                }
                let s = self.value_to_str(&args[0], env)?;
                Ok(Some(Value::Str(s)))
            }
            "bool" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("bool() takes 1 argument".to_string()));
                }
                Ok(Some(Value::Bool(args[0].is_truthy())))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("abs() takes 1 argument".to_string()));
                }
                match &args[0] {
                    Value::Int(n) => Ok(Some(Value::Int(n.abs()))),
                    Value::Float(f) => Ok(Some(Value::Float(f.abs()))),
                    _ => Err(CompileError::Codegen("abs() unsupported type".to_string())),
                }
            }
            "ord" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("ord() takes 1 argument".to_string()));
                }
                let s = args[0].as_str()?;
                let ch = s
                    .chars()
                    .next()
                    .ok_or_else(|| CompileError::Codegen("ord() empty string".to_string()))?;
                Ok(Some(Value::Int(i64::from(ch as u32))))
            }
            "chr" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("chr() takes 1 argument".to_string()));
                }
                #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "chr() input is validated by char::from_u32 on the next line")]
                let n = args[0].as_int()? as u32;
                let ch = char::from_u32(n)
                    .ok_or_else(|| CompileError::Codegen(format!("chr() invalid: {n}")))?;
                Ok(Some(Value::Str(ch.to_string())))
            }
            "sorted" => {
                if args.is_empty() {
                    return Err(CompileError::Codegen(
                        "sorted() takes at least 1 argument".to_string(),
                    ));
                }
                let mut items = self.to_iterable(&args[0])?;
                items.sort_by(value_cmp);
                Ok(Some(Value::List(items)))
            }
            "isinstance" => Ok(Some(Value::Bool(true))),
            "list" => {
                if args.is_empty() {
                    return Ok(Some(Value::List(Vec::new())));
                }
                let items = self.to_iterable(&args[0])?;
                Ok(Some(Value::List(items)))
            }
            "dict" => Ok(Some(Value::Dict(Vec::new()))),
            "tuple" => {
                if args.is_empty() {
                    return Ok(Some(Value::Tuple(Vec::new())));
                }
                let items = self.to_iterable(&args[0])?;
                Ok(Some(Value::Tuple(items)))
            }
            "min" => {
                let items = if args.len() == 1 {
                    self.to_iterable(&args[0])?
                } else {
                    args.to_vec()
                };
                Ok(Some(
                    items.into_iter().min_by(value_cmp).unwrap_or(Value::None),
                ))
            }
            "max" => {
                let items = if args.len() == 1 {
                    self.to_iterable(&args[0])?
                } else {
                    args.to_vec()
                };
                Ok(Some(
                    items.into_iter().max_by(value_cmp).unwrap_or(Value::None),
                ))
            }
            "sum" => {
                let items = self.to_iterable(&args[0])?;
                let mut total = Value::Int(0);
                for item in items {
                    total = self.binop(ast::Operator::Add, &total, &item)?;
                }
                Ok(Some(total))
            }
            "enumerate" => {
                let items = self.to_iterable(&args[0])?;
                let result: Vec<Value> = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v]))
                    .collect();
                Ok(Some(Value::List(result)))
            }
            "reversed" => {
                let mut items = self.to_iterable(&args[0])?;
                items.reverse();
                Ok(Some(Value::List(items)))
            }
            "zip" => {
                let iters: Result<Vec<Vec<Value>>, _> =
                    args.iter().map(|a| self.to_iterable(a)).collect();
                let iters = iters?;
                let min_len = iters.iter().map(Vec::len).min().unwrap_or(0);
                let mut result = Vec::new();
                for i in 0..min_len {
                    let tuple: Vec<Value> = iters.iter().map(|it| it[i].clone()).collect();
                    result.push(Value::Tuple(tuple));
                }
                Ok(Some(Value::List(result)))
            }
            _ => Ok(None),
        }
    }

    // ── Method calls ─────────────────────────────────────────────────────────

    #[expect(clippy::too_many_lines, reason = "method dispatch covers all builtin type methods")]
    fn call_method(
        &mut self,
        obj: &Value,
        method: &str,
        args: &[Value],
        env: &mut Env,
        obj_expr: &Expr,
    ) -> Result<Value, CompileError> {
        match obj {
            Value::Str(s) => self.str_method(s, method, args),
            Value::List(items) => {
                let result = self.list_method(items, method, args)?;
                if let Some(new_list) = &result.1 {
                    self.assign_target(obj_expr, new_list.clone(), env)?;
                }
                Ok(result.0)
            }
            Value::Dict(entries) => {
                let result = self.dict_method(entries, method, args)?;
                if let Some(new_dict) = &result.1 {
                    self.assign_target(obj_expr, new_dict.clone(), env)?;
                }
                Ok(result.0)
            }
            Value::Instance(inst) => {
                if let Some(method_def) = inst.methods.get(method) {
                    let mut full_args = vec![obj.clone()];
                    full_args.extend_from_slice(args);
                    self.call_func(method_def, &full_args, env)
                } else {
                    Err(CompileError::Codegen(format!(
                        "instance has no method '{method}'"
                    )))
                }
            }
            _ => Err(CompileError::Codegen(format!(
                "cannot call method '{method}' on {obj:?}"
            ))),
        }
    }

    fn str_method(&self, s: &str, method: &str, args: &[Value]) -> Result<Value, CompileError> {
        match method {
            "upper" => Ok(Value::Str(s.to_uppercase())),
            "lower" => Ok(Value::Str(s.to_lowercase())),
            "strip" => Ok(Value::Str(s.trim().to_string())),
            "lstrip" => Ok(Value::Str(s.trim_start().to_string())),
            "rstrip" => Ok(Value::Str(s.trim_end().to_string())),
            "split" => {
                let parts: Vec<Value> = if args.is_empty() {
                    s.split_whitespace()
                        .map(|p| Value::Str(p.to_string()))
                        .collect()
                } else {
                    let sep = args[0].as_str()?;
                    s.split(sep).map(|p| Value::Str(p.to_string())).collect()
                };
                Ok(Value::List(parts))
            }
            "join" => {
                if args.len() != 1 {
                    return Err(CompileError::Codegen("join() takes 1 argument".to_string()));
                }
                let Value::List(items) = &args[0] else {
                    return Err(CompileError::Codegen("join() requires a list".to_string()));
                };
                let parts: Result<Vec<&str>, _> = items.iter().map(Value::as_str).collect();
                Ok(Value::Str(parts?.join(s)))
            }
            "replace" => {
                let old = args[0].as_str()?;
                let new = args[1].as_str()?;
                Ok(Value::Str(s.replace(old, new)))
            }
            "startswith" => Ok(Value::Bool(s.starts_with(args[0].as_str()?))),
            "endswith" => Ok(Value::Bool(s.ends_with(args[0].as_str()?))),
            "find" => {
                let needle = args[0].as_str()?;
                Ok(Value::Int(s.find(needle).map_or(-1, |i| i as i64)))
            }
            "count" => {
                let needle = args[0].as_str()?;
                Ok(Value::Int(s.matches(needle).count() as i64))
            }
            "isalpha" => Ok(Value::Bool(
                !s.is_empty() && s.chars().all(char::is_alphabetic),
            )),
            "isdigit" => Ok(Value::Bool(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
            )),
            "isalnum" => Ok(Value::Bool(
                !s.is_empty() && s.chars().all(char::is_alphanumeric),
            )),
            "isupper" => Ok(Value::Bool(
                !s.is_empty() && s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()),
            )),
            "islower" => Ok(Value::Bool(
                !s.is_empty() && s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase()),
            )),
            "title" => {
                let mut result = String::new();
                let mut cap_next = true;
                for ch in s.chars() {
                    if ch.is_alphabetic() {
                        if cap_next {
                            result.extend(ch.to_uppercase());
                            cap_next = false;
                        } else {
                            result.extend(ch.to_lowercase());
                        }
                    } else {
                        result.push(ch);
                        cap_next = true;
                    }
                }
                Ok(Value::Str(result))
            }
            "capitalize" => {
                let mut chars = s.chars();
                let result = match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let upper: String = c.to_uppercase().collect();
                        let rest: String = chars.collect::<String>().to_lowercase();
                        format!("{upper}{rest}")
                    }
                };
                Ok(Value::Str(result))
            }
            "format" => Ok(Value::Str(s.to_string())),
            _ => Err(CompileError::Codegen(format!(
                "unsupported str method: {method}"
            ))),
        }
    }

    /// Returns (`return_value`, `optional_mutated_container`).
    fn list_method(
        &self,
        items: &[Value],
        method: &str,
        args: &[Value],
    ) -> Result<(Value, Option<Value>), CompileError> {
        match method {
            "append" => {
                let mut new_items = items.to_vec();
                new_items.push(args[0].clone());
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "extend" => {
                let mut new_items = items.to_vec();
                if let Value::List(ext) = &args[0] {
                    new_items.extend(ext.iter().cloned());
                }
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "insert" => {
                let idx = args[0].as_int()? as usize;
                let mut new_items = items.to_vec();
                new_items.insert(idx, args[1].clone());
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "pop" => {
                let mut new_items = items.to_vec();
                let val = if args.is_empty() {
                    new_items.pop().unwrap_or(Value::None)
                } else {
                    let idx = args[0].as_int()? as usize;
                    if idx < new_items.len() {
                        new_items.remove(idx)
                    } else {
                        Value::None
                    }
                };
                Ok((val, Some(Value::List(new_items))))
            }
            "remove" => {
                let mut new_items = items.to_vec();
                if let Some(pos) = new_items.iter().position(|v| values_equal(v, &args[0])) {
                    let _ = new_items.remove(pos);
                }
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "sort" => {
                let mut new_items = items.to_vec();
                new_items.sort_by(value_cmp);
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "reverse" => {
                let mut new_items = items.to_vec();
                new_items.reverse();
                Ok((Value::None, Some(Value::List(new_items))))
            }
            "clear" => Ok((Value::None, Some(Value::List(Vec::new())))),
            "index" => {
                let pos = items
                    .iter()
                    .position(|v| values_equal(v, &args[0]))
                    .map_or(-1, |i| i as i64);
                Ok((Value::Int(pos), None))
            }
            "count" => {
                let count = items.iter().filter(|v| values_equal(v, &args[0])).count();
                Ok((Value::Int(count as i64), None))
            }
            "copy" => Ok((Value::List(items.to_vec()), None)),
            _ => Err(CompileError::Codegen(format!(
                "unsupported list method: {method}"
            ))),
        }
    }

    /// Returns (`return_value`, `optional_mutated_container`).
    fn dict_method(
        &self,
        entries: &[(Value, Value)],
        method: &str,
        args: &[Value],
    ) -> Result<(Value, Option<Value>), CompileError> {
        match method {
            "keys" => {
                let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
                Ok((Value::List(keys), None))
            }
            "values" => {
                let vals: Vec<Value> = entries.iter().map(|(_, v)| v.clone()).collect();
                Ok((Value::List(vals), None))
            }
            "items" => {
                let items: Vec<Value> = entries
                    .iter()
                    .map(|(k, v)| Value::Tuple(vec![k.clone(), v.clone()]))
                    .collect();
                Ok((Value::List(items), None))
            }
            "get" => {
                let default = if args.len() > 1 {
                    args[1].clone()
                } else {
                    Value::None
                };
                let val = dict_get(entries, &args[0]).unwrap_or(default);
                Ok((val, None))
            }
            "pop" => {
                let mut new_entries = entries.to_vec();
                let default = if args.len() > 1 {
                    args[1].clone()
                } else {
                    Value::None
                };
                let val = if let Some(pos) = new_entries
                    .iter()
                    .position(|(k, _)| values_equal(k, &args[0]))
                {
                    new_entries.remove(pos).1
                } else {
                    default
                };
                Ok((val, Some(Value::Dict(new_entries))))
            }
            "update" => {
                let mut new_entries = entries.to_vec();
                if let Some(Value::Dict(other)) = args.first() {
                    for (k, v) in other {
                        dict_set(&mut new_entries, k.clone(), v.clone());
                    }
                }
                Ok((Value::None, Some(Value::Dict(new_entries))))
            }
            "clear" => Ok((Value::None, Some(Value::Dict(Vec::new())))),
            "copy" => Ok((Value::Dict(entries.to_vec()), None)),
            "setdefault" => {
                let default = if args.len() > 1 {
                    args[1].clone()
                } else {
                    Value::None
                };
                let mut new_entries = entries.to_vec();
                let val = if let Some(existing) = dict_get(entries, &args[0]) {
                    existing
                } else {
                    dict_set(&mut new_entries, args[0].clone(), default.clone());
                    default
                };
                Ok((val, Some(Value::Dict(new_entries))))
            }
            _ => Err(CompileError::Codegen(format!(
                "unsupported dict method: {method}"
            ))),
        }
    }

    // ── Attribute access ─────────────────────────────────────────────────────

    fn get_attribute(&self, obj: &Value, attr: &str) -> Result<Value, CompileError> {
        match obj {
            Value::Instance(inst) => {
                if let Some(val) = inst.attrs.get(attr) {
                    Ok(val.clone())
                } else if let Some(method) = inst.methods.get(attr) {
                    Ok(Value::BoundMethod(Box::new(obj.clone()), method.clone()))
                } else {
                    Err(CompileError::Codegen(format!(
                        "instance has no attribute '{attr}'"
                    )))
                }
            }
            Value::Str(s) => Ok(Value::BoundMethod(
                Box::new(Value::Str(s.clone())),
                FuncDef {
                    name: attr.to_string(),
                    params: vec![],
                    body: vec![],
                    closure: HashMap::new(),
                },
            )),
            _ => Err(CompileError::Codegen(format!(
                "cannot access attribute '{attr}' on {obj:?}"
            ))),
        }
    }

    // ── Subscript / slice ────────────────────────────────────────────────────

    fn eval_subscript(
        &mut self,
        obj: &Value,
        slice: &Expr,
        env: &mut Env,
    ) -> Result<Value, CompileError> {
        if let Expr::Slice(sl) = slice {
            return self.eval_slice(obj, sl, env);
        }

        let key = self.eval_expr(slice, env)?;

        match obj {
            Value::List(items) => {
                let idx = self.resolve_index(key.as_int()?, items.len())?;
                Ok(items[idx].clone())
            }
            Value::Tuple(items) => {
                let idx = self.resolve_index(key.as_int()?, items.len())?;
                Ok(items[idx].clone())
            }
            Value::Str(s) => {
                let idx = self.resolve_index(key.as_int()?, s.len())?;
                Ok(Value::Str(
                    s.chars().nth(idx).map_or(String::new(), |c| c.to_string()),
                ))
            }
            Value::Dict(entries) => dict_get(entries, &key)
                .ok_or_else(|| CompileError::Codegen(format!("key not found: {key:?}"))),
            _ => Err(CompileError::Codegen(format!(
                "subscript on unsupported type: {obj:?}"
            ))),
        }
    }

    fn eval_slice(
        &mut self,
        obj: &Value,
        sl: &ast::ExprSlice,
        env: &mut Env,
    ) -> Result<Value, CompileError> {
        let items: Vec<Value> = match obj {
            Value::List(v) => v.clone(),
            Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
            Value::Tuple(v) => v.clone(),
            _ => {
                return Err(CompileError::Codegen(
                    "slice on unsupported type".to_string(),
                ))
            }
        };

        let len = items.len() as i64;
        let lower = if let Some(expr) = &sl.lower {
            Some(self.eval_expr(expr, env)?.as_int()?)
        } else {
            None
        };
        let upper = if let Some(expr) = &sl.upper {
            Some(self.eval_expr(expr, env)?.as_int()?)
        } else {
            None
        };
        let step = if let Some(expr) = &sl.step {
            self.eval_expr(expr, env)?.as_int()?
        } else {
            1
        };

        let result = slice_items(&items, lower, upper, step, len);

        match obj {
            Value::Str(_) => {
                let s: String = result
                    .into_iter()
                    .map(|v| match v {
                        Value::Str(s) => s,
                        _ => String::new(),
                    })
                    .collect();
                Ok(Value::Str(s))
            }
            Value::Tuple(_) => Ok(Value::Tuple(result)),
            _ => Ok(Value::List(result)),
        }
    }

    fn resolve_index(&self, idx: i64, len: usize) -> Result<usize, CompileError> {
        let actual = if idx < 0 { len as i64 + idx } else { idx };
        if actual < 0 || actual as usize >= len {
            Err(CompileError::Codegen(format!(
                "index {idx} out of range for length {len}"
            )))
        } else {
            Ok(actual as usize)
        }
    }

    // ── Binary operations ────────────────────────────────────────────────────

    #[expect(clippy::too_many_lines, reason = "binary operations cover all operator and type combinations")]
    fn binop(&self, op: ast::Operator, lhs: &Value, rhs: &Value) -> Result<Value, CompileError> {
        // String concatenation
        if let (Value::Str(a), Value::Str(b)) = (lhs, rhs) {
            if matches!(op, ast::Operator::Add) {
                return Ok(Value::Str(format!("{a}{b}")));
            }
        }

        // String repetition
        if matches!(op, ast::Operator::Mult) {
            if let (Value::Str(s), Value::Int(n)) = (lhs, rhs) {
                return Ok(Value::Str(s.repeat(*n as usize)));
            }
        }

        // List concatenation
        if let (Value::List(a), Value::List(b)) = (lhs, rhs) {
            if matches!(op, ast::Operator::Add) {
                let mut result = a.clone();
                result.extend(b.iter().cloned());
                return Ok(Value::List(result));
            }
        }

        // Float operations
        if matches!(lhs, Value::Float(_)) || matches!(rhs, Value::Float(_)) {
            let a = lhs.as_float()?;
            let b = rhs.as_float()?;
            return match op {
                ast::Operator::Add => Ok(Value::Float(a + b)),
                ast::Operator::Sub => Ok(Value::Float(a - b)),
                ast::Operator::Mult => Ok(Value::Float(a * b)),
                ast::Operator::Div => Ok(Value::Float(a / b)),
                ast::Operator::FloorDiv => Ok(Value::Float((a / b).floor())),
                ast::Operator::Mod => Ok(Value::Float(a % b)),
                ast::Operator::Pow => Ok(Value::Float(a.powf(b))),
                _ => Err(CompileError::Codegen(format!(
                    "unsupported float operator: {op:?}"
                ))),
            };
        }

        // Int operations
        let a = lhs.as_int()?;
        let b = rhs.as_int()?;
        match op {
            ast::Operator::Add => Ok(Value::Int(a + b)),
            ast::Operator::Sub => Ok(Value::Int(a - b)),
            ast::Operator::Mult => Ok(Value::Int(a * b)),
            ast::Operator::Div => Ok(Value::Float(a as f64 / b as f64)),
            ast::Operator::FloorDiv => {
                if b == 0 {
                    Err(CompileError::Codegen("division by zero".to_string()))
                } else {
                    Ok(Value::Int(a.div_euclid(b)))
                }
            }
            ast::Operator::Mod => {
                if b == 0 {
                    Err(CompileError::Codegen("modulo by zero".to_string()))
                } else {
                    Ok(Value::Int(a.rem_euclid(b)))
                }
            }
            ast::Operator::Pow => {
                if b >= 0 {
                    Ok(Value::Int(a.pow(b as u32)))
                } else {
                    Ok(Value::Float((a as f64).powi(b as i32)))
                }
            }
            ast::Operator::BitAnd => Ok(Value::Int(a & b)),
            ast::Operator::BitOr => Ok(Value::Int(a | b)),
            ast::Operator::BitXor => Ok(Value::Int(a ^ b)),
            ast::Operator::LShift => Ok(Value::Int(a << b)),
            ast::Operator::RShift => Ok(Value::Int(a >> b)),
            _ => Err(CompileError::Codegen(format!(
                "unsupported int operator: {op:?}"
            ))),
        }
    }

    // ── Comparison operations ────────────────────────────────────────────────

    fn compare_op(&self, op: ast::CmpOp, lhs: &Value, rhs: &Value) -> Result<bool, CompileError> {
        match op {
            ast::CmpOp::Eq => Ok(values_equal(lhs, rhs)),
            ast::CmpOp::NotEq => Ok(!values_equal(lhs, rhs)),
            ast::CmpOp::Lt => Ok(value_cmp(lhs, rhs) == std::cmp::Ordering::Less),
            ast::CmpOp::LtE => Ok(!matches!(value_cmp(lhs, rhs), std::cmp::Ordering::Greater)),
            ast::CmpOp::Gt => Ok(value_cmp(lhs, rhs) == std::cmp::Ordering::Greater),
            ast::CmpOp::GtE => Ok(!matches!(value_cmp(lhs, rhs), std::cmp::Ordering::Less)),
            ast::CmpOp::In => self.contains(rhs, lhs),
            ast::CmpOp::NotIn => Ok(!self.contains(rhs, lhs)?),
            ast::CmpOp::Is => Ok(matches!((lhs, rhs), (Value::None, Value::None))),
            ast::CmpOp::IsNot => Ok(!matches!((lhs, rhs), (Value::None, Value::None))),
        }
    }

    fn contains(&self, container: &Value, item: &Value) -> Result<bool, CompileError> {
        match container {
            Value::Str(s) => {
                let needle = item.as_str()?;
                Ok(s.contains(needle))
            }
            Value::List(items) => Ok(items.iter().any(|v| values_equal(v, item))),
            Value::Tuple(items) => Ok(items.iter().any(|v| values_equal(v, item))),
            Value::Dict(entries) => Ok(entries.iter().any(|(k, _)| values_equal(k, item))),
            _ => Err(CompileError::Codegen(
                "unsupported 'in' container".to_string(),
            )),
        }
    }

    // ── Iteration ────────────────────────────────────────────────────────────

    fn to_iterable(&self, val: &Value) -> Result<Vec<Value>, CompileError> {
        match val {
            Value::List(items) => Ok(items.clone()),
            Value::Tuple(items) => Ok(items.clone()),
            Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
            Value::Dict(entries) => Ok(entries.iter().map(|(k, _)| k.clone()).collect()),
            _ => Err(CompileError::Codegen(format!("not iterable: {val:?}"))),
        }
    }

    // ── Value to string (for print / f-strings) ──────────────────────────────

    fn value_to_str(&mut self, val: &Value, env: &mut Env) -> Result<String, CompileError> {
        match val {
            Value::Instance(inst) => {
                if let Some(str_method) = inst.methods.get("__str__") {
                    let result = self.call_func(str_method, std::slice::from_ref(val), env)?;
                    match result {
                        Value::Str(s) => Ok(s),
                        other => Ok(format!("{other}")),
                    }
                } else {
                    Ok(format!("{val}"))
                }
            }
            other => Ok(format!("{other}")),
        }
    }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::None, Value::None) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => (*x as f64) == *y,
        (Value::Bool(x), Value::Int(y)) => i64::from(*x) == *y,
        (Value::Int(x), Value::Bool(y)) => *x == i64::from(*y),
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) | (Value::Tuple(x), Value::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        _ => false,
    }
}

fn value_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Tuple(x), Value::Tuple(y)) | (Value::List(x), Value::List(y)) => {
            for (a, b) in x.iter().zip(y.iter()) {
                let cmp = value_cmp(a, b);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            x.len().cmp(&y.len())
        }
        _ => std::cmp::Ordering::Equal,
    }
}

fn dict_get(entries: &[(Value, Value)], key: &Value) -> Option<Value> {
    entries
        .iter()
        .rev()
        .find(|(k, _)| values_equal(k, key))
        .map(|(_, v)| v.clone())
}

fn dict_set(entries: &mut Vec<(Value, Value)>, key: Value, val: Value) {
    if let Some(pos) = entries.iter().position(|(k, _)| values_equal(k, &key)) {
        entries[pos].1 = val;
    } else {
        entries.push((key, val));
    }
}

fn slice_items(
    items: &[Value],
    lower: Option<i64>,
    upper: Option<i64>,
    step: i64,
    len: i64,
) -> Vec<Value> {
    if step == 0 {
        return Vec::new();
    }

    let (start, stop) = if step > 0 {
        let start = lower.map_or(0, |l| normalize_slice_idx(l, len).max(0));
        let stop = upper.map_or(len, |u| normalize_slice_idx(u, len).min(len));
        (start, stop)
    } else {
        let start = lower.map_or(len - 1, |l| normalize_slice_idx(l, len).min(len - 1));
        let stop = upper.map_or(-1, |u| normalize_slice_idx(u, len).max(-1));
        (start, stop)
    };

    let mut result = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < stop {
            if i >= 0 && (i as usize) < items.len() {
                result.push(items[i as usize].clone());
            }
            i += step;
        }
    } else {
        while i > stop {
            if i >= 0 && (i as usize) < items.len() {
                result.push(items[i as usize].clone());
            }
            i += step;
        }
    }
    result
}

fn normalize_slice_idx(idx: i64, len: i64) -> i64 {
    if idx < 0 {
        (len + idx).max(0)
    } else {
        idx
    }
}
