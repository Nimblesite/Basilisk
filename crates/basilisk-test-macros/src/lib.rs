//! Test-only procedural macros for Basilisk.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

struct MutationSafeArgs {
    rule: LitStr,
}

impl Parse for MutationSafeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "rule" {
            return Err(syn::Error::new_spanned(key, "expected `rule`"));
        }

        let _eq = input.parse::<Token![=]>()?;
        let rule = input.parse::<LitStr>()?;

        if input.peek(Token![,]) {
            let _comma = input.parse::<Token![,]>()?;
        }

        if !input.is_empty() {
            return Err(input.error("expected only `rule = \"eNNNN\"`"));
        }

        let rule_value = rule.value();
        if !is_rule_code(&rule_value) {
            return Err(syn::Error::new_spanned(
                rule,
                "rule must match the Basilisk rule form `eNNNN`",
            ));
        }

        Ok(Self { rule })
    }
}

/// Marks a Rust test as safe for the mutation-test Make target.
///
/// The original test remains unchanged. When compiled with
/// `--cfg mutation_testing`, this macro also emits a generated wrapper test
/// whose module path starts with `mutation_safe_<rule>_`, which lets the Make
/// target select marked tests through Rust's built-in test-name filter.
#[proc_macro_attribute]
pub fn mutation_safe(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attribute as MutationSafeArgs);
    let function = parse_macro_input!(item as ItemFn);

    expand_mutation_safe(&args, &function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_mutation_safe(
    args: &MutationSafeArgs,
    function: &ItemFn,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    validate_test_function(function)?;

    let rule = args.rule.value();
    let function_name = &function.sig.ident;
    let wrapper_module = format_ident!("mutation_safe_{rule}_{function_name}");
    let output = &function.sig.output;

    Ok(quote! {
        #function

        #[cfg(mutation_testing)]
        mod #wrapper_module {
            #[test]
            fn #function_name() #output {
                super::#function_name()
            }
        }
    })
}

fn validate_test_function(function: &ItemFn) -> Result<(), syn::Error> {
    if !function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("test"))
    {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "`#[mutation_safe]` must be used on a `#[test]` function",
        ));
    }

    if function.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "`#[mutation_safe]` does not support async tests",
        ));
    }

    if function.sig.constness.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.ident,
            "`#[mutation_safe]` does not support const functions",
        ));
    }

    if !function.sig.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "`#[mutation_safe]` tests must not take arguments",
        ));
    }

    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "`#[mutation_safe]` tests must not be generic",
        ));
    }

    Ok(())
}

fn is_rule_code(value: &str) -> bool {
    matches!(
        value.as_bytes(),
        [b'e', first, second, third, fourth]
            if first.is_ascii_digit()
                && second.is_ascii_digit()
                && third.is_ascii_digit()
                && fourth.is_ascii_digit()
    )
}
