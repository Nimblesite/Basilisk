//! Implements [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Semantic token parsing and assertion helpers.

/// Parse semantic token data into Vec of (deltaLine, deltaStart, length, tokenType, modifiers).
#[must_use]
pub fn parse_semantic_tokens(data: &[serde_json::Value]) -> Vec<Vec<u64>> {
    data.chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect()
}

/// Assert that semantic token data is well-formed (multiple of 5, non-negative values).
///
/// # Panics
///
/// Panics if the data length is not a multiple of 5 or if any value is negative.
pub fn assert_valid_semantic_token_data(data: &[serde_json::Value], resp: &str) {
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
    for (idx, value) in data.iter().enumerate() {
        let num = value.as_i64().unwrap_or(-1);
        assert!(
            num >= 0,
            "semantic token data[{idx}] must be non-negative, got {num}: {resp}"
        );
    }
}
