pub use basilisk_checker::{check, Diagnostic};
pub use basilisk_parser::parse_source;
pub use basilisk_resolver::resolve;

pub fn run(source: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

pub fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

pub fn codes_owned(diags: &[Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.code.to_string()).collect()
}

pub fn has_code(diags: &[Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code.code == code)
}

pub fn messages_for<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a str> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.as_str())
        .collect()
}
