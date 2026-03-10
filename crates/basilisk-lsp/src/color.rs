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
        if bytes[idx] == b'"' || bytes[idx] == b'\'' {
            let quote = bytes[idx];
            idx += 1;
            // Scan the string body for `#` followed by hex digits.
            while idx < len && bytes[idx] != quote {
                if bytes[idx] == b'\\' {
                    idx += 2; // skip escaped char
                    continue;
                }
                if bytes[idx] == b'#' {
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
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let r = (color.red * 255.0_f32) as u8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let g = (color.green * 255.0_f32) as u8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let b = (color.blue * 255.0_f32) as u8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

/// Try to parse a hex color starting at `start` in `bytes`.
///
/// Returns `(Color, hex_digit_count)` on success.
/// Supports 3-digit (`#RGB`), 6-digit (`#RRGGBB`), and 8-digit (`#RRGGBBAA`).
fn parse_hex_color(bytes: &[u8], start: usize) -> Option<(Color, usize)> {
    // Count consecutive hex digits.
    let mut count = 0;
    while start + count < bytes.len() && is_hex_digit(bytes[start + count]) {
        count += 1;
    }

    match count {
        3 => {
            let r = hex_val(bytes[start])?;
            let g = hex_val(bytes[start + 1])?;
            let b = hex_val(bytes[start + 2])?;
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
            let r = hex_byte(bytes[start], bytes[start + 1])?;
            let g = hex_byte(bytes[start + 2], bytes[start + 3])?;
            let b = hex_byte(bytes[start + 4], bytes[start + 5])?;
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
            let r = hex_byte(bytes[start], bytes[start + 1])?;
            let g = hex_byte(bytes[start + 2], bytes[start + 3])?;
            let b = hex_byte(bytes[start + 4], bytes[start + 5])?;
            let a = hex_byte(bytes[start + 6], bytes[start + 7])?;
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
