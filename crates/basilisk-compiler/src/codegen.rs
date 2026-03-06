//! Cranelift-based JIT code generation.
//!
//! Currently supports:
//! - Function definitions with `int` params and `int`/`None` return
//! - `print(string_literal)` and `print(int_expr)` calls
//! - Integer literals, binary arithmetic (`+`, `*`)
//! - `return` statements
//! - Top-level expression statements (function calls)

use basilisk_resolver::scope::{FunctionInfo, ResolvedModule, ReturnAnnotationKind};
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashMap;

use crate::CompileError;

/// Name of the synthetic entry-point function that holds top-level code.
const ENTRY_NAME: &str = "__basilisk_main__";

/// JIT-compile and execute a parsed module, returning captured stdout.
///
/// Uses type information from the resolver's [`ResolvedModule`] to build
/// correct function signatures (parameter counts, return types) rather than
/// re-parsing AST annotations.
///
/// # Errors
///
/// Returns `CompileError` if JIT compilation or execution fails.
pub fn jit_compile_and_run(
    module: &ast::ModModule,
    resolved: &ResolvedModule,
) -> Result<String, CompileError> {
    let mut compiler = JitCompiler::new().map_err(|err| {
        CompileError::Codegen(format!("failed to create JIT compiler: {err}"))
    })?;

    compiler.compile_module(module, resolved)?;
    compiler.execute()
}

/// Function signature info for user-defined functions.
struct FuncInfo {
    /// Cranelift function ID.
    id: FuncId,
    /// Number of i64 parameters.
    param_count: usize,
    /// Whether the function returns an i64 (vs void).
    returns_int: bool,
}

/// Minimal JIT compiler backed by Cranelift.
struct JitCompiler {
    /// The Cranelift JIT module.
    jit_module: JITModule,
    /// Compiled function info by name.
    functions: HashMap<String, FuncInfo>,
    /// String constants to embed in memory (kept alive for the JIT's lifetime).
    string_constants: Vec<String>,
}

impl JitCompiler {
    fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        // On aarch64, PIC/PLT is not supported in older cranelift JIT.
        if cfg!(target_arch = "x86_64") {
            flag_builder
                .set("is_pic", "true")
                .map_err(|err| err.to_string())?;
            flag_builder
                .set("use_colocated_libcalls", "false")
                .map_err(|err| err.to_string())?;
        }

        let isa_builder =
            cranelift_native::builder().map_err(|msg| format!("native ISA error: {msg}"))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|err| err.to_string())?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Register runtime helpers as extern symbols.
        builder.symbol("basilisk_print_str", basilisk_print_str as *const u8);
        builder.symbol("basilisk_print_int", basilisk_print_int as *const u8);

        let jit_module = JITModule::new(builder);

        Ok(Self {
            jit_module,
            functions: HashMap::new(),
            string_constants: Vec::new(),
        })
    }

    /// Compile all top-level statements in the module.
    ///
    /// Uses resolved [`FunctionInfo`] from the resolver for type-aware
    /// signature building instead of re-parsing AST annotations.
    fn compile_module(
        &mut self,
        module: &ast::ModModule,
        resolved: &ResolvedModule,
    ) -> Result<(), CompileError> {
        // Build a lookup from function name → resolved FunctionInfo
        let resolved_funcs: HashMap<&str, &FunctionInfo> = resolved
            .functions
            .iter()
            .map(|fi| (fi.name.as_str(), fi))
            .collect();

        // First pass: declare all user-defined functions with signatures from resolver
        for stmt in &module.body {
            if let Stmt::FunctionDef(func_def) = stmt {
                let name = func_def.name.as_str();
                let func_info = resolved_funcs.get(name).ok_or_else(|| {
                    CompileError::Codegen(format!(
                        "function '{name}' not found in resolved module"
                    ))
                })?;
                self.declare_function(name, func_info)?;
            }
        }

        // Second pass: define function bodies
        for stmt in &module.body {
            if let Stmt::FunctionDef(func_def) = stmt {
                self.compile_function(func_def)?;
            }
        }

        // Third pass: compile all top-level non-function statements into entry
        let top_level_stmts: Vec<&Stmt> = module
            .body
            .iter()
            .filter(|s| !matches!(s, Stmt::FunctionDef(_)))
            .collect();

        if !top_level_stmts.is_empty() {
            self.compile_entry(&top_level_stmts)?;
        }

        self.jit_module.finalize_definitions().map_err(|err| {
            CompileError::Codegen(format!("failed to finalize: {err}"))
        })?;

        Ok(())
    }

    /// Declare a function using type info from the resolver's [`FunctionInfo`].
    fn declare_function(
        &mut self,
        name: &str,
        func_info: &FunctionInfo,
    ) -> Result<(), CompileError> {
        let param_count = func_info.parameters.len();
        let returns_int = func_info.return_annotation == ReturnAnnotationKind::Other;

        let mut sig = self.jit_module.make_signature();
        for _ in 0..param_count {
            sig.params.push(AbiParam::new(types::I64));
        }
        if returns_int {
            sig.returns.push(AbiParam::new(types::I64));
        }

        let func_id = self
            .jit_module
            .declare_function(name, Linkage::Local, &sig)
            .map_err(|err| CompileError::Codegen(format!("declare {name}: {err}")))?;

        self.functions.insert(
            name.to_string(),
            CraneliftFuncInfo {
                id: func_id,
                param_count,
                returns_int,
            },
        );
        Ok(())
    }

    /// Compile a user-defined function body.
    fn compile_function(&mut self, func_def: &ast::StmtFunctionDef) -> Result<(), CompileError> {
        let name = func_def.name.as_str();
        let info = &self.functions[name];
        let func_id = info.id;
        let param_count = info.param_count;
        let returns_int = info.returns_int;

        let mut sig = self.jit_module.make_signature();
        for _ in 0..param_count {
            sig.params.push(AbiParam::new(types::I64));
        }
        if returns_int {
            sig.returns.push(AbiParam::new(types::I64));
        }

        let mut ctx = self.jit_module.make_context();
        ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Map parameter names to cranelift values
        let mut locals: HashMap<String, Value> = HashMap::new();
        for (idx, param) in func_def.parameters.args.iter().enumerate() {
            let val = builder.block_params(entry_block)[idx];
            locals.insert(param.parameter.name.to_string(), val);
        }

        let mut returned = false;
        for stmt in &func_def.body {
            if returned {
                break;
            }
            if let Stmt::Return(ret) = stmt {
                if let Some(val_expr) = &ret.value {
                    let val = self.compile_expr(&mut builder, val_expr, &locals)?;
                    builder.ins().return_(&[val]);
                } else {
                    builder.ins().return_(&[]);
                }
                returned = true;
            } else {
                self.compile_stmt(&mut builder, stmt, &locals)?;
            }
        }

        if !returned {
            builder.ins().return_(&[]);
        }

        builder.finalize();

        self.jit_module
            .define_function(func_id, &mut ctx)
            .map_err(|err| CompileError::Codegen(format!("define {name}: {err}")))?;

        self.jit_module.clear_context(&mut ctx);
        Ok(())
    }

    /// Compile top-level statements into a synthetic entry function.
    fn compile_entry(&mut self, stmts: &[&Stmt]) -> Result<(), CompileError> {
        let sig = self.jit_module.make_signature();
        let func_id = self
            .jit_module
            .declare_function(ENTRY_NAME, Linkage::Local, &sig)
            .map_err(|err| CompileError::Codegen(format!("declare entry: {err}")))?;
        self.functions.insert(
            ENTRY_NAME.to_string(),
            FuncInfo {
                id: func_id,
                param_count: 0,
                returns_int: false,
            },
        );

        let mut ctx = self.jit_module.make_context();
        ctx.func.signature = self.jit_module.make_signature();

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let locals = HashMap::new();
        for stmt in stmts {
            self.compile_stmt(&mut builder, stmt, &locals)?;
        }

        builder.ins().return_(&[]);
        builder.finalize();

        self.jit_module
            .define_function(func_id, &mut ctx)
            .map_err(|err| CompileError::Codegen(format!("define entry: {err}")))?;

        self.jit_module.clear_context(&mut ctx);
        Ok(())
    }

    /// Compile a single statement.
    fn compile_stmt(
        &mut self,
        builder: &mut FunctionBuilder,
        stmt: &Stmt,
        locals: &HashMap<String, Value>,
    ) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Call(call) = expr_stmt.value.as_ref() {
                    // Discard return value for expression statements
                    let _ = self.compile_call(builder, call, locals)?;
                }
                Ok(())
            }
            _ => Err(CompileError::Codegen(format!(
                "unsupported statement: {stmt:?}"
            ))),
        }
    }

    /// Compile an expression, returning the cranelift Value.
    fn compile_expr(
        &mut self,
        builder: &mut FunctionBuilder,
        expr: &Expr,
        locals: &HashMap<String, Value>,
    ) -> Result<Value, CompileError> {
        match expr {
            Expr::NumberLiteral(num) => {
                let val = match &num.value {
                    ast::Number::Int(int_val) => int_val
                        .as_i64()
                        .ok_or_else(|| CompileError::Codegen("int too large".to_string()))?,
                    _ => {
                        return Err(CompileError::Codegen(
                            "only int literals supported".to_string(),
                        ))
                    }
                };
                Ok(builder.ins().iconst(types::I64, val))
            }
            Expr::Name(name) => {
                let val = locals.get(name.id.as_str()).ok_or_else(|| {
                    CompileError::Codegen(format!("undefined variable: {}", name.id))
                })?;
                Ok(*val)
            }
            Expr::BinOp(binop) => {
                let lhs = self.compile_expr(builder, &binop.left, locals)?;
                let rhs = self.compile_expr(builder, &binop.right, locals)?;
                match binop.op {
                    ast::Operator::Add => Ok(builder.ins().iadd(lhs, rhs)),
                    ast::Operator::Sub => Ok(builder.ins().isub(lhs, rhs)),
                    ast::Operator::Mult => Ok(builder.ins().imul(lhs, rhs)),
                    _ => Err(CompileError::Codegen(format!(
                        "unsupported operator: {:?}",
                        binop.op
                    ))),
                }
            }
            Expr::Call(call) => self.compile_call(builder, call, locals),
            _ => Err(CompileError::Codegen(format!(
                "unsupported expression: {expr:?}"
            ))),
        }
    }

    /// Compile a function call, returning the result Value (i64 or dummy).
    fn compile_call(
        &mut self,
        builder: &mut FunctionBuilder,
        call: &ast::ExprCall,
        locals: &HashMap<String, Value>,
    ) -> Result<Value, CompileError> {
        let func_name = match call.func.as_ref() {
            Expr::Name(name) => name.id.as_str(),
            _ => return Err(CompileError::Codegen("unsupported call target".to_string())),
        };

        if func_name == "print" {
            self.compile_print(builder, call, locals)?;
            return Ok(builder.ins().iconst(types::I64, 0));
        }

        // Call a user-defined function
        let info = self.functions.get(func_name).ok_or_else(|| {
            CompileError::Codegen(format!("undefined function: {func_name}"))
        })?;
        let callee_id = info.id;
        let returns_int = info.returns_int;

        // Build the callee signature
        let mut sig = self.jit_module.make_signature();
        for _ in &call.arguments.args {
            sig.params.push(AbiParam::new(types::I64));
        }
        if returns_int {
            sig.returns.push(AbiParam::new(types::I64));
        }

        let callee_ref = self
            .jit_module
            .declare_func_in_func(callee_id, builder.func);

        // Compile arguments
        let mut args = Vec::new();
        for arg in &call.arguments.args {
            args.push(self.compile_expr(builder, arg, locals)?);
        }

        let inst = builder.ins().call(callee_ref, &args);

        if returns_int {
            Ok(builder.inst_results(inst)[0])
        } else {
            Ok(builder.ins().iconst(types::I64, 0))
        }
    }

    /// Compile `print(expr)` — string literal or integer expression.
    fn compile_print(
        &mut self,
        builder: &mut FunctionBuilder,
        call: &ast::ExprCall,
        locals: &HashMap<String, Value>,
    ) -> Result<(), CompileError> {
        if call.arguments.args.len() != 1 {
            return Err(CompileError::Codegen(
                "print() with != 1 arg not yet supported".to_string(),
            ));
        }

        let arg = &call.arguments.args[0];

        // String literal path
        if let Expr::StringLiteral(s) = arg {
            let string_val = s.value.to_str().to_string();
            let len = string_val.len();
            self.string_constants.push(string_val);
            let string_ref = &self.string_constants[self.string_constants.len() - 1];
            let ptr_val = string_ref.as_ptr() as i64;

            let pointer_type = self.jit_module.target_config().pointer_type();
            let mut sig = self.jit_module.make_signature();
            sig.params.push(AbiParam::new(pointer_type));
            sig.params.push(AbiParam::new(pointer_type));

            let print_func = self
                .jit_module
                .declare_function("basilisk_print_str", Linkage::Import, &sig)
                .map_err(|err| CompileError::Codegen(format!("declare print_str: {err}")))?;

            let print_ref = self
                .jit_module
                .declare_func_in_func(print_func, builder.func);

            let ptr = builder.ins().iconst(pointer_type, ptr_val);
            #[allow(clippy::cast_possible_wrap)]
            let len_val = builder.ins().iconst(pointer_type, len as i64);
            builder.ins().call(print_ref, &[ptr, len_val]);

            return Ok(());
        }

        // Integer expression path
        let val = self.compile_expr(builder, arg, locals)?;

        let mut sig = self.jit_module.make_signature();
        sig.params.push(AbiParam::new(types::I64));

        let print_func = self
            .jit_module
            .declare_function("basilisk_print_int", Linkage::Import, &sig)
            .map_err(|err| CompileError::Codegen(format!("declare print_int: {err}")))?;

        let print_ref = self
            .jit_module
            .declare_func_in_func(print_func, builder.func);

        builder.ins().call(print_ref, &[val]);
        Ok(())
    }

    /// Execute the entry function and capture stdout.
    fn execute(&self) -> Result<String, CompileError> {
        let info = self.functions.get(ENTRY_NAME).ok_or_else(|| {
            CompileError::Codegen("no top-level code to execute".to_string())
        })?;

        let output = std::sync::Arc::new(std::sync::Mutex::new(String::new()));

        OUTPUT_BUFFER.with(|buf| {
            *buf.borrow_mut() = Some(std::sync::Arc::clone(&output));
        });

        let code_ptr = self.jit_module.get_finalized_function(info.id);

        // SAFETY: we generated this function with signature () -> void
        #[allow(unsafe_code)]
        let func = unsafe { std::mem::transmute::<*const u8, fn()>(code_ptr) };
        func();

        OUTPUT_BUFFER.with(|buf| {
            *buf.borrow_mut() = None;
        });

        let result = output
            .lock()
            .map_err(|err| CompileError::Codegen(format!("lock error: {err}")))?;
        Ok(result.clone())
    }
}

// --- Runtime support ---

thread_local! {
    static OUTPUT_BUFFER: std::cell::RefCell<Option<std::sync::Arc<std::sync::Mutex<String>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Write to the output buffer (shared by all print runtime functions).
fn write_output(text: &str) {
    OUTPUT_BUFFER.with(|buf| {
        if let Some(output) = buf.borrow().as_ref() {
            if let Ok(mut out) = output.lock() {
                out.push_str(text);
                out.push('\n');
            }
        }
    });
}

/// Runtime: print a string (ptr + len).
///
/// # Safety
///
/// Called from JIT-compiled code with valid pointer and length.
#[allow(unsafe_code)]
extern "C" fn basilisk_print_str(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(bytes);
    write_output(&text);
}

/// Runtime: print an i64.
extern "C" fn basilisk_print_int(val: i64) {
    write_output(&val.to_string());
}
