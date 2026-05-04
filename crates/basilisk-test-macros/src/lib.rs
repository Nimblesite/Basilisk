//! Test-only procedural macros for Basilisk.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

struct MutationSafeArgs {
    rule: LitStr,
    /// Pipe-separated function names to scope mutant selection, e.g. `"fn_a|fn_b"`.
    /// When present, the Makefile builds `--re "fn_a|fn_b"` instead of the whole file.
    fns: Option<LitStr>,
}

impl Parse for MutationSafeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "rule" {
            return Err(syn::Error::new_spanned(key, "expected `rule`"));
        }
        let _eq = input.parse::<Token![=]>()?;
        let rule = input.parse::<LitStr>()?;

        let rule_value = rule.value();
        if !is_rule_code(&rule_value) {
            return Err(syn::Error::new_spanned(
                rule,
                "rule must match the Basilisk rule form `eNNNN`",
            ));
        }

        let fns = if input.peek(Token![,]) {
            let _comma = input.parse::<Token![,]>()?;
            let fns_key: Ident = input.parse()?;
            if fns_key != "fns" {
                return Err(syn::Error::new_spanned(fns_key, "expected `fns`"));
            }
            let _eq = input.parse::<Token![=]>()?;
            let fns_val = input.parse::<LitStr>()?;
            if input.peek(Token![,]) {
                let _comma = input.parse::<Token![,]>()?;
            }
            Some(fns_val)
        } else {
            None
        };

        if !input.is_empty() {
            return Err(
                input.error("expected `rule = \"eNNNN\"` or `rule = \"eNNNN\", fns = \"fn1|fn2\"`")
            );
        }

        Ok(Self { rule, fns })
    }
}

/// Marks a Rust test as safe for the mutation-test Make target.
///
/// Required: `rule = "eNNNN"` — the rule file this test mutates.
/// Optional: `fns = "fn_a|fn_b"` — pipe-separated function names to scope the
/// mutant regex to specific functions, dramatically reducing the mutant count.
///
/// The original test remains unchanged. When compiled with `--cfg mutation_testing`,
/// this macro also emits a wrapper test whose module path encodes the rule and
/// targeted functions so the Make target can extract both from `--list` output.
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
    let output = &function.sig.output;

    // Encode targeted functions into the module name so the Makefile can extract them.
    // Module name: `mutation_safe_{rule}_{fns_slug}_{test_fn}` when fns is given,
    //              `mutation_safe_{rule}_{test_fn}` otherwise.
    // fns_slug: pipe chars replaced with `__` for a valid Rust identifier.
    let wrapper_module = match &args.fns {
        Some(fns_lit) => {
            let slug = fns_lit.value().replace('|', "__");
            format_ident!("mutation_safe_{rule}_fns__{slug}__{function_name}")
        }
        None => format_ident!("mutation_safe_{rule}_{function_name}"),
    };

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
