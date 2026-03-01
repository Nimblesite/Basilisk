//! Rule E0093: Flow union type inference
//!
//! This rule implements flow-sensitive type inference that tracks variable
//! assignments across control flow branches and infers union types when
//! variables are assigned different types in different code paths.

use basilisk_resolver::{ResolvedModule, VariableInfo};
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::inference::{infer_variable_type, infer_flow_union_types};
use crate::rules::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0093",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0093",
};

/// Rule for flow union type inference.
pub struct FlowUnionInference;

impl Rule for FlowUnionInference {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all variable assignments across the module
        let mut assignments = Vec::new();
        
        // Process module-level variables
        for var_info in &module.variables {
            let inferred_type = infer_variable_type(var_info);
            assignments.push((var_info.name.clone(), inferred_type));
        }
        
        // Process function-local variables
        for func_info in &module.functions {
            for var_info in &func_info.variables {
                let inferred_type = infer_variable_type(var_info);
                assignments.push((var_info.name.clone(), inferred_type));
            }
        }
        
        // Process class-level variables
        for class_info in &module.classes {
            for var_info in &class_info.variables {
                let inferred_type = infer_variable_type(var_info);
                assignments.push((var_info.name.clone(), inferred_type));
            }
        }
        
        // Infer union types for variables assigned in different code paths
        let union_types = infer_flow_union_types(&assignments);
        
        // For now, we'll just log the inferred union types for debugging
        // In a full implementation, we would use these types for further analysis
        for (var_name, union_type) in union_types {
            // Skip variables with single types (no union needed)
            if let crate::types::InferredType::Union(types) = &union_type {
                if types.len() > 1 {
                    // This variable has been assigned multiple types across code paths
                    // We could add diagnostics here for type safety violations
                    // For now, we'll just track the union type internally
                }
            }
        }
        
        // Check annotated variables for type compatibility
        check_annotated_variables(module, diagnostics);
    }
}

/// Checks annotated variables for type compatibility with their assignments.
fn check_annotated_variables(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    // Check module-level annotated variables
    for var_info in &module.variables {
        if var_info.has_annotation {
            if let Err(err) = crate::inference::check_annotated_variable(var_info) {
                diagnostics.push(Diagnostic {
                    code: ErrorCode::E0093,
                    severity: Severity::Error,
                    message: format!("Annotated variable '{}' has incompatible assignment: {}", var_info.name, err),
                    span: var_info.name_span,
                });
            }
        }
    }
    
    // Check function-local annotated variables
    for func_info in &module.functions {
        for var_info in &func_info.variables {
            if var_info.has_annotation {
                if let Err(err) = crate::inference::check_annotated_variable(var_info) {
                    diagnostics.push(Diagnostic {
                        code: ErrorCode::E0093,
                        severity: Severity::Error,
                        message: format!("Annotated variable '{}' has incompatible assignment: {}", var_info.name, err),
                        span: var_info.name_span,
                    });
                }
            }
        }
    }
    
    // Check class-level annotated variables
    for class_info in &module.classes {
        for var_info in &class_info.variables {
            if var_info.has_annotation {
                if let Err(err) = crate::inference::check_annotated_variable(var_info) {
                    diagnostics.push(Diagnostic {
                        code: ErrorCode::E0093,
                        severity: Severity::Error,
                        message: format!("Annotated variable '{}' has incompatible assignment: {}", var_info.name, err),
                        span: var_info.name_span,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use basilisk_resolver::{FunctionInfo, ClassInfo, Span};

    #[test]
    fn test_flow_union_inference_rule() {
        let module = ResolvedModule {
            source: "".to_string(),
            variables: vec![],
            functions: vec![],
            classes: vec![],
            imports: vec![],
            type_aliases: vec![],
            type_vars: vec![],
            type_var_tuples: vec![],
            protocols: vec![],
            enums: vec![],
            named_tuples: vec![],
            typed_dicts: vec![],
            new_types: vec![],
            match_statements: vec![],
            return_statements: vec![],
            module_assignments: vec![],
            module_accesses: vec![],
            module_comparisons: vec![],
            final_violations: vec![],
            read_only_violations: vec![],
            historical_positional_violations: vec![],
            enum_value_type_violations: vec![],
            literal_string_enum_mismatches: vec![],
            float_param_int_attr_accesses: vec![],
            local_class_var_violations: vec![],
            invalid_string_annotations: vec![],
            pep695_bound_violations: vec![],
            protocol_self_violations: vec![],
            annotated_too_few_args: vec![],
            typed_dict_key_violations: vec![],
            typed_dict_second_arg_kinds: vec![],
            unhashable_key_refs: vec![],
            named_tuple_defs: vec![],
            type_statements: vec![],
            type_var_calls: vec![],
            type_alias_type_calls: vec![],
            new_type_calls: vec![],
            typed_dict_calls: vec![],
            reveal_type_calls: vec![],
            assert_type_calls: vec![],
        };
        
        let mut diagnostics = Vec::new();
        let rule = FlowUnionInference;
        
        rule.check(&module, &mut diagnostics);
        
        // Should not produce any diagnostics for empty module
        assert!(diagnostics.is_empty());
    }
}