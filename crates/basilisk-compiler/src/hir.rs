//! Implements [COMPARCH]. See docs/specs/COMPILER-ARCHITECTURE-SPEC.md#COMPARCH
//! High-level Intermediate Representation (HIR).
//!
//! UNIMPLEMENTED roadmap stub for [COMPILER-HIR]. The spec calls for a typed,
//! monomorphized IR bridging the analyzer's `ResolvedModule` and LLVM codegen
//! (resolving `InferredType` to concrete layouts, monomorphizing generics,
//! lowering unions to tagged structs, protocols to vtables, etc.). None of that
//! exists yet — this file is an empty placeholder, and the interpreter in
//! `codegen.rs` walks the AST directly without any HIR. Listed here only so
//! `grep [COMPILER-HIR]` surfaces the gap; this is NOT an implementation.
