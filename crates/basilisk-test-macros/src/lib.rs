//! Implements [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
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
        if !is_rule_slug(&rule_value) {
            return Err(syn::Error::new_spanned(
                rule,
                "rule must be a rule path slug: lowercase ASCII starting with a \
                 letter, e.g. `assignment_compatibility` or `aliases_implicit`",
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
            return Err(input.error(
                "expected `rule = \"<slug>\"` or `rule = \"<slug>\", fns = \"fn1|fn2\"`",
            ));
        }

        Ok(Self { rule, fns })
    }
}

/// Marks a Rust test as safe for the mutation-test Make target.
///
/// Required: `rule = "<slug>"` — the rule's path stem under
/// `crates/basilisk-checker/src/rules/` (a file like `aliases_implicit` or a
/// directory like `assignment_compatibility`). This is what scopes mutant
/// selection to that rule's source.
/// Optional: `fns = "fn_a|fn_b"` — pipe-separated function names to scope the
/// mutant regex to specific functions, dramatically reducing the mutant count.
/// Omitting `fns` scopes the whole rule file (encoded via `WHOLE_FILE_SLUG`).
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

    // Encode the rule slug and targeted functions into the wrapper module name so
    // `scripts/mutation_examine_re.py` can recover both from `cargo test --list`.
    // Both forms share the `_fns__{slug}__` delimiter — a whole-file test uses the
    // `WHOLE_FILE_SLUG` sentinel for `{slug}` — so the parser has a single,
    // unambiguous format regardless of how many underscores the descriptive rule
    // slug contains (`mutation_safe_{rule}_fns__{slug}__{test_fn}`).
    // fns_slug: pipe chars replaced with `__` for a valid Rust identifier.
    let fns_slug = match &args.fns {
        Some(fns_lit) => fns_lit.value().replace('|', "__"),
        None => WHOLE_FILE_SLUG.to_owned(),
    };
    let wrapper_module = format_ident!("mutation_safe_{rule}_fns__{fns_slug}__{function_name}");

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

/// Sentinel `{slug}` value used in the wrapper-module name when a test is scoped to
/// a whole rule file (no `fns`). `scripts/mutation_examine_re.py` recognises this
/// exact string and emits a whole-file mutant pattern instead of per-function ones.
/// Keep the two in sync. It is a single lowercase token so the generated module name
/// stays `snake_case` and cannot collide with a real rule slug or function name.
const WHOLE_FILE_SLUG: &str = "wholefile";

/// A rule slug is the path stem of a rule under `crates/basilisk-checker/src/rules/`
/// (a file like `aliases_implicit` or a directory like `assignment_compatibility`).
/// It must be a valid Rust identifier fragment — a lowercase ASCII letter first,
/// then lowercase ASCII letters, digits, or underscores — so it can be embedded in
/// the generated wrapper-module identifier.
fn is_rule_slug(value: &str) -> bool {
    let mut bytes = value.bytes();
    let starts_with_lower_letter = matches!(bytes.next(), Some(first) if first.is_ascii_lowercase());
    starts_with_lower_letter
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
