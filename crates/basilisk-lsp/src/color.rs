//! Implements [LSPARCH-FEATURES]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES
//!
//! Document Color provider: detect hex color strings in Python source.
//!
//! Scans string literals for CSS-style hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`)
//! and returns `ColorInformation` entries so VS Code shows color swatches inline.

use tower_lsp::lsp_types::{Color, ColorInformation, ColorPresentation, Range};

use crate::util::byte_offset_to_position;

/// Find all hex color strings in the source text.
#[must_use]
pub fn document_colors(source: &str) -> Vec<ColorInformation> {
    let mut colors = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut idx = 0;

    while idx < len {
        // Look for string delimiters.
        let Some(&current) = bytes.get(idx) else {
            break;
        };
        if current == b'"' || current == b'\'' {
            let quote = current;
            idx += 1;
            // Scan the string body for `#` followed by hex digits.
            while idx < len {
                let Some(&ch) = bytes.get(idx) else {
                    break;
                };
                if ch == quote {
                    break;
                }
                if ch == b'\\' {
                    idx += 2; // skip escaped char
                    continue;
                }
                if ch == b'#' {
                    if let Some((color, hex_len)) = parse_hex_color(bytes, idx + 1) {
                        let start = byte_offset_to_position(source, idx);
                        let end = byte_offset_to_position(source, idx + 1 + hex_len);
                        colors.push(ColorInformation {
                            range: Range { start, end },
                            color,
                        });
                        idx += 1 + hex_len;
                        continue;
                    }
                }
                idx += 1;
            }
        }
        idx += 1;
    }

    colors
}

/// Build color presentation strings from an RGBA color value.
#[must_use]
pub fn color_presentations(color: &Color, range: &Range) -> Vec<ColorPresentation> {
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "LSP Color components are 0.0..=1.0 so f32*255 fits in u8"
    )]
    let r = (color.red * 255.0_f32) as u8;
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "LSP Color components are 0.0..=1.0 so f32*255 fits in u8"
    )]
    let g = (color.green * 255.0_f32) as u8;
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "LSP Color components are 0.0..=1.0 so f32*255 fits in u8"
    )]
    let b = (color.blue * 255.0_f32) as u8;
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "LSP Color components are 0.0..=1.0 so f32*255 fits in u8"
    )]
    let a = (color.alpha * 255.0_f32) as u8;

    let mut presentations = Vec::with_capacity(2);

    // 6-digit hex (no alpha or fully opaque).
    if a == 255 {
        presentations.push(ColorPresentation {
            label: format!("#{r:02x}{g:02x}{b:02x}"),
            text_edit: Some(tower_lsp::lsp_types::TextEdit {
                range: *range,
                new_text: format!("#{r:02x}{g:02x}{b:02x}"),
            }),
            additional_text_edits: None,
        });
    }

    // 8-digit hex (with alpha).
    presentations.push(ColorPresentation {
        label: format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        text_edit: Some(tower_lsp::lsp_types::TextEdit {
            range: *range,
            new_text: format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        }),
        additional_text_edits: None,
    });

    presentations
}

/// Parse the first six hex digits of `hex_bytes` into `(r, g, b)` byte values.
fn parse_rgb(hex_bytes: &[u8]) -> Option<(u8, u8, u8)> {
    let r = hex_byte(*hex_bytes.first()?, *hex_bytes.get(1)?)?;
    let g = hex_byte(*hex_bytes.get(2)?, *hex_bytes.get(3)?)?;
    let b = hex_byte(*hex_bytes.get(4)?, *hex_bytes.get(5)?)?;
    Some((r, g, b))
}

/// Try to parse a hex color starting at `start` in `bytes`.
///
/// Returns `(Color, hex_digit_count)` on success.
/// Supports 3-digit (`#RGB`), 6-digit (`#RRGGBB`), and 8-digit (`#RRGGBBAA`).
fn parse_hex_color(bytes: &[u8], start: usize) -> Option<(Color, usize)> {
    // Count consecutive hex digits.
    let hex_bytes = bytes.get(start..)?;
    let count = hex_bytes.iter().take_while(|&&b| is_hex_digit(b)).count();

    match count {
        3 => {
            let r = hex_val(*hex_bytes.first()?)?;
            let g = hex_val(*hex_bytes.get(1)?)?;
            let b = hex_val(*hex_bytes.get(2)?)?;
            Some((
                Color {
                    red: f32::from(r * 17) / 255.0_f32,
                    green: f32::from(g * 17) / 255.0_f32,
                    blue: f32::from(b * 17) / 255.0_f32,
                    alpha: 1.0_f32,
                },
                3,
            ))
        }
        6 => {
            let (r, g, b) = parse_rgb(hex_bytes)?;
            Some((
                Color {
                    red: f32::from(r) / 255.0_f32,
                    green: f32::from(g) / 255.0_f32,
                    blue: f32::from(b) / 255.0_f32,
                    alpha: 1.0_f32,
                },
                6,
            ))
        }
        8 => {
            let (r, g, b) = parse_rgb(hex_bytes)?;
            let a = hex_byte(*hex_bytes.get(6)?, *hex_bytes.get(7)?)?;
            Some((
                Color {
                    red: f32::from(r) / 255.0_f32,
                    green: f32::from(g) / 255.0_f32,
                    blue: f32::from(b) / 255.0_f32,
                    alpha: f32::from(a) / 255.0_f32,
                },
                8,
            ))
        }
        _ => None,
    }
}

fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_byte(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_val(hi)? * 16 + hex_val(lo)?)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    const EPS: f32 = 0.001;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn no_color_source_returns_empty() {
        assert!(document_colors("x = 1\ny = 'hello world'\n").is_empty());
    }

    #[test]
    fn hash_without_hex_is_ignored() {
        // `#` not followed by a valid hex-length run.
        assert!(document_colors("s = '# not a color'").is_empty());
        // `#` at end of string.
        assert!(document_colors("s = 'trailing #'").is_empty());
    }

    #[test]
    fn hash_outside_string_is_ignored() {
        // A `#` comment outside any string literal must not be scanned.
        assert!(document_colors("x = 1  # ff0000 looks hex but isn't a string").is_empty());
    }

    #[test]
    fn short_and_invalid_hex_ignored() {
        // 2-digit run: not 3/6/8.
        assert!(document_colors("s = '#ab'").is_empty());
        // 4-digit run: not a supported length.
        assert!(document_colors("s = '#abcd'").is_empty());
        // 5-digit run.
        assert!(document_colors("s = '#abcde'").is_empty());
        // 7-digit run.
        assert!(document_colors("s = '#abcdef0'").is_empty());
        // `#` followed by non-hex.
        assert!(document_colors("s = '#zzz'").is_empty());
    }

    #[test]
    fn six_digit_double_quote_red() {
        let colors = document_colors("c = \"#ff0000\"");
        assert_eq!(colors.len(), 1);
        let color = colors[0].color;
        assert!(approx(color.red, 1.0));
        assert!(approx(color.green, 0.0));
        assert!(approx(color.blue, 0.0));
        assert!(approx(color.alpha, 1.0));
    }

    #[test]
    fn six_digit_single_quote_green() {
        let colors = document_colors("c = '#00ff00'");
        assert_eq!(colors.len(), 1);
        let color = colors[0].color;
        assert!(approx(color.red, 0.0));
        assert!(approx(color.green, 1.0));
        assert!(approx(color.blue, 0.0));
        assert!(approx(color.alpha, 1.0));
    }

    #[test]
    fn six_digit_range_spans_hash_through_last_digit() {
        // `c = "#ff0000"` — the `#` is at byte offset 5, span covers 7 bytes.
        let source = "c = \"#ff0000\"";
        let colors = document_colors(source);
        assert_eq!(colors.len(), 1);
        let range = colors[0].range;
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 5);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 12);
    }

    #[test]
    fn three_digit_expands_to_full_byte() {
        // `#f00` → red channel f -> 0xff, green/blue 0.
        let colors = document_colors("c = '#f00'");
        assert_eq!(colors.len(), 1);
        let color = colors[0].color;
        assert!(approx(color.red, 1.0));
        assert!(approx(color.green, 0.0));
        assert!(approx(color.blue, 0.0));
        assert!(approx(color.alpha, 1.0));
        // Span covers `#` + 3 digits = 4 bytes.
        assert_eq!(colors[0].range.start.character, 5);
        assert_eq!(colors[0].range.end.character, 9);
    }

    #[test]
    fn three_digit_mid_nibble_expands() {
        // `#abc` → a->0xaa, b->0xbb, c->0xcc.
        let colors = document_colors("c = '#abc'");
        assert_eq!(colors.len(), 1);
        let color = colors[0].color;
        assert!(approx(color.red, f32::from(0xaa_u8) / 255.0));
        assert!(approx(color.green, f32::from(0xbb_u8) / 255.0));
        assert!(approx(color.blue, f32::from(0xcc_u8) / 255.0));
    }

    #[test]
    fn eight_digit_carries_alpha() {
        // `#ff000080` → red 1.0, alpha 0x80/255.
        let colors = document_colors("c = '#ff000080'");
        assert_eq!(colors.len(), 1);
        let color = colors[0].color;
        assert!(approx(color.red, 1.0));
        assert!(approx(color.green, 0.0));
        assert!(approx(color.blue, 0.0));
        assert!(approx(color.alpha, f32::from(0x80_u8) / 255.0));
        // Span covers `#` + 8 digits = 9 bytes.
        assert_eq!(colors[0].range.start.character, 5);
        assert_eq!(colors[0].range.end.character, 14);
    }

    #[test]
    fn uppercase_hex_digits_parse() {
        let colors = document_colors("c = '#00FF00'");
        assert_eq!(colors.len(), 1);
        assert!(approx(colors[0].color.green, 1.0));
    }

    #[test]
    fn multiple_colors_on_one_line() {
        let colors = document_colors("c = '#ff0000 and #0000ff'");
        assert_eq!(colors.len(), 2);
        assert!(approx(colors[0].color.red, 1.0));
        assert!(approx(colors[1].color.blue, 1.0));
    }

    #[test]
    fn colors_in_different_literals() {
        let source = "a = '#ff0000'\nb = \"#00ff00\"\n";
        let colors = document_colors(source);
        assert_eq!(colors.len(), 2);
        assert_eq!(colors[0].range.start.line, 0);
        assert_eq!(colors[1].range.start.line, 1);
        assert!(approx(colors[0].color.red, 1.0));
        assert!(approx(colors[1].color.green, 1.0));
    }

    #[test]
    fn escaped_char_skipped_in_string() {
        // The backslash escape advances two bytes; the color after it still parses.
        let colors = document_colors("s = 'a\\t#00ff00'");
        assert_eq!(colors.len(), 1);
        assert!(approx(colors[0].color.green, 1.0));
    }

    #[test]
    fn presentations_opaque_returns_six_and_eight_digit() {
        let color = Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        };
        let range = Range::default();
        let presentations = color_presentations(&color, &range);
        assert_eq!(presentations.len(), 2);
        assert_eq!(presentations[0].label, "#ff0000");
        assert_eq!(presentations[1].label, "#ff0000ff");
        // Both carry a text edit with matching new_text.
        assert_eq!(
            presentations[0]
                .text_edit
                .as_ref()
                .map(|e| e.new_text.clone()),
            Some("#ff0000".to_owned())
        );
        assert_eq!(
            presentations[1]
                .text_edit
                .as_ref()
                .map(|e| e.new_text.clone()),
            Some("#ff0000ff".to_owned())
        );
    }

    #[test]
    fn presentations_translucent_returns_only_eight_digit() {
        let color = Color {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
            alpha: f32::from(0x80_u8) / 255.0,
        };
        let range = Range::default();
        let presentations = color_presentations(&color, &range);
        assert_eq!(presentations.len(), 1);
        assert_eq!(presentations[0].label, "#0000ff80");
    }

    #[test]
    fn parse_hex_color_rejects_bad_lengths() {
        // Directly exercise the helper's rejected branches.
        assert!(parse_hex_color(b"ab", 0).is_none());
        assert!(parse_hex_color(b"abcd", 0).is_none());
        assert!(parse_hex_color(b"", 0).is_none());
    }

    #[test]
    fn hex_val_and_hex_byte_cover_ranges() {
        assert_eq!(hex_val(b'0'), Some(0));
        assert_eq!(hex_val(b'9'), Some(9));
        assert_eq!(hex_val(b'a'), Some(10));
        assert_eq!(hex_val(b'F'), Some(15));
        assert_eq!(hex_val(b'g'), None);
        assert_eq!(hex_byte(b'f', b'f'), Some(255));
        assert_eq!(hex_byte(b'0', b'0'), Some(0));
        assert_eq!(hex_byte(b'z', b'0'), None);
    }
}
