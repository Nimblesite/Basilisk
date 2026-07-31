//! Tests [STUBRES-TYPESHED-BUILTINS-INDEX] — the drift gate binding the
//! committed artifact to the live parser, plus codec roundtrip and
//! corruption-rejection coverage for `super` (`builtins_index.rs`).

use super::*;

/// THE drift gate: the committed artifact must be byte-identical to a fresh
/// extraction by the real parser over the embedded bundle. A typeshed bundle
/// refresh without `cargo run -p basilisk-stubs --bin gen_builtins_index`
/// fails here.
#[test]
fn embedded_index_matches_regenerated_bytes() -> Result<(), BuiltinsIndexError> {
    let regenerated = regenerate()?;
    assert_eq!(
        regenerated.as_slice(),
        EMBEDDED_INDEX,
        "data/typeshed/builtins_index.bin is stale — regenerate with \
         `cargo run -p basilisk-stubs --bin gen_builtins_index`"
    );
    Ok(())
}

/// Semantic anchor: decoding the artifact yields EXACTLY the class map the
/// live no-target extraction produces — the two code paths that
/// `shared_builtins_index` treats as interchangeable.
#[test]
fn embedded_index_decodes_to_live_extraction() -> Result<(), BuiltinsIndexError> {
    let snapshot = bundle::bundled_snapshot()?;
    let (logical_uri, source_text) = snapshot
        .read_stub("builtins")
        .ok_or(BuiltinsIndexError::MissingBuiltins)?;
    let module = crate::parse_pyi_source(
        source_text,
        std::path::Path::new(&logical_uri),
        "builtins",
        crate::StubSource::Typeshed,
        crate::StubTier::Tier1,
    )
    .map_err(|error| BuiltinsIndexError::Parse(error.to_string()))?;
    let (sha, decoded) = decode_classes(EMBEDDED_INDEX)?;
    assert_eq!(sha, bundle::manifest_bundle_sha()?);
    assert_eq!(decoded, module.classes);
    Ok(())
}

/// The public loader serves the artifact for the current bundle and the map
/// holds the classes the checker leans on hardest.
#[test]
fn bundled_builtins_classes_serves_core_builtin_classes() {
    let classes = bundled_builtins_classes();
    let Some(classes) = classes else {
        assert!(classes.is_some(), "embedded artifact must decode");
        return;
    };
    for name in ["object", "int", "str", "list", "dict", "type"] {
        assert!(classes.contains_key(name), "missing builtin class {name}");
    }
    let int_class = classes.get("int");
    assert!(
        int_class.is_some_and(|class| class
            .methods
            .iter()
            .any(|method| method.name == "bit_length")),
        "int must keep its extracted methods"
    );
}

fn synthetic_classes() -> HashMap<String, StubClass> {
    let receiver = StubParam {
        name: "self".to_owned(),
        annotation: None,
        has_default: false,
        kind: StubParamKind::Regular,
    };
    let params = vec![
        StubParam {
            name: "value".to_owned(),
            annotation: Some("int | None".to_owned()),
            has_default: true,
            kind: StubParamKind::PositionalOnly,
        },
        StubParam {
            name: "args".to_owned(),
            annotation: None,
            has_default: false,
            kind: StubParamKind::Vararg,
        },
        StubParam {
            name: "flag".to_owned(),
            annotation: Some("bool".to_owned()),
            has_default: false,
            kind: StubParamKind::KeywordOnly,
        },
        StubParam {
            name: "extra".to_owned(),
            annotation: Some("object".to_owned()),
            has_default: false,
            kind: StubParamKind::Kwarg,
        },
    ];
    let method = StubFunction {
        name: "configure".to_owned(),
        receiver: Some(receiver),
        params,
        return_type: Some("Widget".to_owned()),
        is_overload: true,
        is_async: true,
        decorators: vec!["overload".to_owned(), "deprecated".to_owned()],
        class_name: Some("Widget".to_owned()),
        source_span: StubSpan { start: 17, end: 26 },
    };
    let static_method = StubFunction {
        name: "make".to_owned(),
        receiver: None,
        params: Vec::new(),
        return_type: None,
        is_overload: false,
        is_async: false,
        decorators: vec!["staticmethod".to_owned()],
        class_name: Some("Widget".to_owned()),
        source_span: StubSpan { start: 40, end: 44 },
    };
    let widget = StubClass {
        name: "Widget".to_owned(),
        bases: vec!["Base".to_owned(), "Protocol[int]".to_owned()],
        metaclass: Some("ABCMeta".to_owned()),
        methods: vec![method, static_method],
        attributes: vec![
            StubVariable {
                name: "count".to_owned(),
                annotation: Some("int".to_owned()),
            },
            StubVariable {
                name: "bare".to_owned(),
                annotation: None,
            },
        ],
    };
    let empty = StubClass {
        name: "Empty".to_owned(),
        bases: Vec::new(),
        metaclass: None,
        methods: Vec::new(),
        attributes: Vec::new(),
    };
    [("Widget".to_owned(), widget), ("Empty".to_owned(), empty)]
        .into_iter()
        .collect()
}

#[test]
fn roundtrip_preserves_every_field_shape() -> Result<(), BuiltinsIndexError> {
    let classes = synthetic_classes();
    let encoded = encode_classes(&classes, "deadbeef")?;
    let (sha, decoded) = decode_classes(&encoded)?;
    assert_eq!(sha, "deadbeef");
    assert_eq!(decoded, classes);
    Ok(())
}

#[test]
fn encoding_is_deterministic_across_map_orderings() -> Result<(), BuiltinsIndexError> {
    let classes = synthetic_classes();
    let mut pairs: Vec<(String, StubClass)> = classes
        .iter()
        .map(|(name, class)| (name.clone(), class.clone()))
        .collect();
    pairs.sort_by(|left, right| right.0.cmp(&left.0));
    let reversed: HashMap<String, StubClass> = pairs.into_iter().collect();
    assert_eq!(
        encode_classes(&classes, "cafe")?,
        encode_classes(&reversed, "cafe")?
    );
    Ok(())
}

#[test]
fn malformed_artifacts_are_rejected_not_panicked() -> Result<(), BuiltinsIndexError> {
    assert!(decode_classes(&[]).is_err(), "empty input must be rejected");
    assert!(
        decode_classes(b"NOTMAGIC").is_err(),
        "wrong magic must be rejected"
    );
    let valid = encode_classes(&synthetic_classes(), "beef")?;
    for cut in [1, MAGIC.len(), valid.len() / 2, valid.len() - 1] {
        let truncated = valid.get(..cut).unwrap_or(&[]);
        assert!(
            decode_classes(truncated).is_err(),
            "truncation at {cut} must be rejected"
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(
        decode_classes(&trailing).is_err(),
        "trailing bytes must be rejected"
    );
    // Layout walk to the first class's metaclass tag: magic (8) + sha "beef"
    // (4-byte length + 4 bytes) + class count (4) + name "Empty" (4 + 5) +
    // bases count (4) = byte 33. An option tag outside {0, 1} is malformed.
    let mut bad_tag = valid;
    if let Some(tag) = bad_tag.get_mut(33) {
        *tag = 9;
    }
    assert!(
        decode_classes(&bad_tag).is_err(),
        "corrupted option tag must be rejected"
    );
    Ok(())
}
