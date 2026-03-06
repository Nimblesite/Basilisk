//! Cranelift-based JIT code generation.
//!
//! Currently supports a tiny subset:
//! - Function definitions with `-> None` return
//! - `print(string_literal)` calls
//! - Top-level function calls

use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashMap;

use crate::CompileError;

/// Name of the synthetic entry-point function that holds top-level code.
const ENTRY_NAME: &str = "__basilisk_main__";

/// JIT-compile and execute a parsed module, returning captured stdout.
pub fn jit_compile_and_run(module: &ast::ModModule) -> Result<String, CompileError> {
    let mut compiler = JitCompiler::new().map_err(|err| {
        CompileError::Codegen(format!("failed to create JIT compiler: {err}"))
    })?;

    compiler.compile_module(module)?;
    compiler.execute()
}

/// Minimal JIT compiler backed by Cranelift.
struct JitCompiler {
    /// The Cranelift JIT module.
    jit_module: JITModule,
    /// Compiled function IDs by name.
    functions: HashMap<String, FuncId>,
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

        // Register our print helper as an extern symbol.
        builder.symbol("basilisk_print", basilisk_print as *const u8);

        let jit_module = JITModule::new(builder);

        Ok(Self {
            jit_module,
            functions: HashMap::new(),
            string_constants: Vec::new(),
        })
    }

    /// Compile all top-level statements in the module.
    fn compile_module(&mut self, module: &ast::ModModule) -> Result<(), CompileError> {
        // First pass: declare all user-defined functions
        for stmt in &module.body {
            if let Stmt::FunctionDef(func_def) = stmt {
                self.declare_function(&func_def.name)?;
            }
        }

        // Second pass: define function bodies
        for stmt in &module.body {
            if let Stmt::FunctionDef(func_def) = stmt {
                self.compile_function(func_def)?;
            }
        }

        // Third pass: compile all top-level non-function statements into __basilisk_main__
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

    /// Declare a function signature (all user functions are `() -> void` for now).
    fn declare_function(&mut self, name: &str) -> Result<(), CompileError> {
        let sig = self.jit_module.make_signature();

        let func_id = self
            .jit_module
            .declare_function(name, Linkage::Local, &sig)
            .map_err(|err| CompileError::Codegen(format!("declare {name}: {err}")))?;

        self.functions.insert(name.to_string(), func_id);
        Ok(())
    }

    /// Compile a user-defined function body.
    fn compile_function(&mut self, func_def: &ast::StmtFunctionDef) -> Result<(), CompileError> {
        let name = func_def.name.as_str();
        let func_id = self.functions[name];

        let mut ctx = self.jit_module.make_context();
        ctx.func.signature = self.jit_module.make_signature();

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        for stmt in &func_def.body {
            self.compile_stmt(&mut builder, stmt)?;
        }

        builder.ins().return_(&[]);
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
        self.functions.insert(ENTRY_NAME.to_string(), func_id);

        let mut ctx = self.jit_module.make_context();
        ctx.func.signature = self.jit_module.make_signature();

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        for stmt in stmts {
            self.compile_stmt(&mut builder, stmt)?;
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
    ) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Call(call) = expr_stmt.value.as_ref() {
                    self.compile_call(builder, call)?;
                }
                Ok(())
            }
            _ => Err(CompileError::Codegen(format!(
                "unsupported statement: {:?}",
                std::mem::discriminant(stmt)
            ))),
        }
    }

    /// Compile a function call expression.
    fn compile_call(
        &mut self,
        builder: &mut FunctionBuilder,
        call: &ast::ExprCall,
    ) -> Result<(), CompileError> {
        let func_name = match call.func.as_ref() {
            Expr::Name(name) => name.id.as_str(),
            _ => return Err(CompileError::Codegen("unsupported call target".to_string())),
        };

        if func_name == "print" {
            return self.compile_print(builder, call);
        }

        // Call a user-defined function
        let callee_id = self.functions.get(func_name).ok_or_else(|| {
            CompileError::Codegen(format!("undefined function: {func_name}"))
        })?;
        let callee_ref = self
            .jit_module
            .declare_func_in_func(*callee_id, builder.func);
        builder.ins().call(callee_ref, &[]);
        Ok(())
    }

    /// Compile `print(string_literal)` → call to `basilisk_print(ptr, len)`.
    fn compile_print(
        &mut self,
        builder: &mut FunctionBuilder,
        call: &ast::ExprCall,
    ) -> Result<(), CompileError> {
        if call.arguments.args.len() != 1 {
            return Err(CompileError::Codegen(
                "print() with != 1 arg not yet supported".to_string(),
            ));
        }

        let arg = &call.arguments.args[0];
        let string_val = match arg {
            Expr::StringLiteral(s) => s.value.to_str().to_string(),
            _ => {
                return Err(CompileError::Codegen(
                    "print() only supports string literals for now".to_string(),
                ))
            }
        };

        let len = string_val.len();
        self.string_constants.push(string_val);
        let string_ref = &self.string_constants[self.string_constants.len() - 1];
        let ptr_val = string_ref.as_ptr() as i64;

        let pointer_type = self.jit_module.target_config().pointer_type();
        let mut print_sig = self.jit_module.make_signature();
        print_sig.params.push(AbiParam::new(pointer_type));
        print_sig.params.push(AbiParam::new(pointer_type));

        let print_func = self
            .jit_module
            .declare_function("basilisk_print", Linkage::Import, &print_sig)
            .map_err(|err| CompileError::Codegen(format!("declare basilisk_print: {err}")))?;

        let print_ref = self
            .jit_module
            .declare_func_in_func(print_func, builder.func);

        let ptr = builder.ins().iconst(pointer_type, ptr_val);
        let len_val = builder.ins().iconst(pointer_type, len as i64);
        builder.ins().call(print_ref, &[ptr, len_val]);

        Ok(())
    }

    /// Execute the entry function and capture stdout.
    fn execute(&self) -> Result<String, CompileError> {
        let func_id = self.functions.get(ENTRY_NAME).ok_or_else(|| {
            CompileError::Codegen("no top-level code to execute".to_string())
        })?;

        let output = std::sync::Arc::new(std::sync::Mutex::new(String::new()));

        OUTPUT_BUFFER.with(|buf| {
            *buf.borrow_mut() = Some(std::sync::Arc::clone(&output));
        });

        let code_ptr = self.jit_module.get_finalized_function(*func_id);

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

/// Runtime print function called by JIT-compiled code.
///
/// # Safety
///
/// Called from JIT-compiled code with valid pointer and length.
#[allow(unsafe_code)]
extern "C" fn basilisk_print(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(bytes);

    OUTPUT_BUFFER.with(|buf| {
        if let Some(output) = buf.borrow().as_ref() {
            if let Ok(mut out) = output.lock() {
                out.push_str(&text);
                out.push('\n');
            }
        }
    });
}
