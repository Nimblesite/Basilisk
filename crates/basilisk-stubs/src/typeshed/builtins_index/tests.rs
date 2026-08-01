//! Tests [STUBRES-TYPESHED-BUILTINS-INDEX] — the drift gate binding the
//! committed artifact to the live parser, the target-version keying, plus
//! codec roundtrip and corruption-rejection coverage for `super`
//! (`builtins_index.rs` and `builtins_index/codec.rs`).

use super::codec::MAGIC;
use super::*;
use crate::types::{StubFunction, StubParam, StubParamKind, StubSpan, StubVariable};

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

/// Semantic anchor: the artifact serves EXACTLY the class map the live
/// no-target extraction produces — the two code paths that
/// `shared_builtins_index` treats as interchangeable.
#[test]
fn embedded_index_decodes_to_live_untargeted_extraction() -> Result<(), BuiltinsIndexError> {
    let (logical_uri, source_text) = bundled_builtins_source()?;
    let live = extract_untargeted(&logical_uri, source_text)?;
    assert_eq!(bundled_builtins_classes(None), Some(live));
    Ok(())
}

/// The whole point of the target-keyed artifact: a project that pins a Python
/// version is served from the artifact too, and gets the SAME map the live
/// targeted parse would have produced. A regression here silently changes what
/// `builtins` means for every pinned project.
#[test]
fn embedded_index_serves_every_target_minor_exactly() -> Result<(), BuiltinsIndexError> {
    let (logical_uri, source_text) = bundled_builtins_source()?;
    for minor in 0..=MAX_GENERATED_MINOR {
        let target = StubTarget {
            python_version: (3, u32::from(minor)),
            platform: StubTargetPlatform::All,
        };
        let live = extract_for_target(&logical_uri, source_text, minor, &StubTargetPlatform::All)?;
        assert_eq!(
            bundled_builtins_classes(Some(&target)),
            Some(live),
            "artifact must match the live parse for 3.{minor}"
        );
    }
    Ok(())
}

/// The final interval is open-ended, so a target past the generated range must
/// still be served (and served the same map as the last generated minor) —
/// never fall back to a live parse just because Python kept counting.
#[test]
fn targets_past_the_generated_range_reuse_the_final_interval() {
    let last = StubTarget {
        python_version: (3, u32::from(MAX_GENERATED_MINOR)),
        platform: StubTargetPlatform::All,
    };
    let far = StubTarget {
        python_version: (3, 400),
        platform: StubTargetPlatform::All,
    };
    let served = bundled_builtins_classes(Some(&far));
    assert!(served.is_some(), "a far-future 3.x target must be served");
    assert_eq!(served, bundled_builtins_classes(Some(&last)));
}

/// Platform is part of the key, and it must be served exactly: `builtins.pyi`
/// guards `OSError.winerror` behind `sys.platform == "win32"`, so a Windows
/// target has to get that member and a non-Windows one must not. Serving one
/// platform's map to another would silently invent or delete an attribute.
#[test]
fn each_platform_class_serves_its_own_live_map() -> Result<(), BuiltinsIndexError> {
    let (logical_uri, source_text) = bundled_builtins_source()?;
    let map_for = |platform: StubTargetPlatform| {
        bundled_builtins_classes(Some(&StubTarget {
            python_version: (3, 13),
            platform,
        }))
    };
    for platform in [
        StubTargetPlatform::All,
        StubTargetPlatform::Concrete("win32".to_owned()),
        StubTargetPlatform::Concrete("darwin".to_owned()),
        StubTargetPlatform::Concrete("linux".to_owned()),
    ] {
        let live = extract_for_target(&logical_uri, source_text, 13, &platform)?;
        assert_eq!(
            map_for(platform.clone()),
            Some(live),
            "artifact must match the live parse for {platform:?}"
        );
    }
    let has_winerror = |platform: StubTargetPlatform| {
        map_for(platform)
            .and_then(|classes| classes.get("OSError").cloned())
            .is_some_and(|class| class.attributes.iter().any(|a| a.name == "winerror"))
    };
    assert!(
        has_winerror(StubTargetPlatform::Concrete("win32".to_owned())),
        "a win32 target must keep OSError.winerror"
    );
    for platform in [
        StubTargetPlatform::All,
        StubTargetPlatform::Concrete("darwin".to_owned()),
        StubTargetPlatform::Concrete("basilisk-never-a-platform".to_owned()),
    ] {
        assert!(
            !has_winerror(platform.clone()),
            "{platform:?} must not see the win32-only OSError.winerror"
        );
    }
    Ok(())
}

/// A major version the artifact does not model must fall back to live
/// extraction rather than silently serving a 3.x map.
#[test]
fn a_non_three_major_target_falls_back_to_live_extraction() {
    let target = StubTarget {
        python_version: (4, 0),
        platform: StubTargetPlatform::All,
    };
    assert_eq!(bundled_builtins_classes(Some(&target)), None);
}

/// The pool must actually pool: every (platform, version) variant repeats
/// nearly all of the same ~100 classes, so storing them separately would put
/// well over a megabyte of duplicated class bodies inside every shipped
/// binary. Guards the space half of the design.
#[test]
fn pooling_keeps_the_artifact_far_below_its_unpooled_size() -> Result<(), BuiltinsIndexError> {
    let (_, artifact) = codec::decode(EMBEDDED_INDEX)?;
    let variants = artifact.variant_count();
    assert!(variants > 1, "the bundle must produce several variants");
    let unpooled: usize = (0..variants)
        .map(|variant| artifact.pooled_bytes_of_variant(variant))
        .sum::<Result<usize, BuiltinsIndexError>>()?;
    assert!(
        EMBEDDED_INDEX.len() * 3 < unpooled,
        "artifact is {} bytes for {variants} variants whose bodies total {unpooled} bytes — \
         class pooling has stopped working",
        EMBEDDED_INDEX.len()
    );
    Ok(())
}

/// The public loader serves the artifact for the current bundle and the map
/// holds the classes the checker leans on hardest.
#[test]
fn bundled_builtins_classes_serves_core_builtin_classes() {
    let classes = bundled_builtins_classes(None);
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

fn bundled_builtins_source() -> Result<(String, &'static str), BuiltinsIndexError> {
    let snapshot = bundle::bundled_snapshot()?;
    // The bundled snapshot is a process-lifetime `OnceLock` over `'static`
    // embedded bytes, so leaking the clone here costs nothing new and lets the
    // borrowed stub body outlive the temporary.
    let snapshot: &'static _ = Box::leak(Box::new(snapshot));
    snapshot
        .read_stub("builtins")
        .ok_or(BuiltinsIndexError::MissingBuiltins)
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

/// A synthetic artifact in the real one's shape: `Empty` is shared, `Widget`
/// exists only below 3.12, and only the `win32` platform class keeps it at all.
fn synthetic_artifact() -> Artifact {
    let all = synthetic_classes();
    let mut modern = all.clone();
    let _ = modern.remove("Widget");
    Artifact {
        default_classes: modern.clone(),
        groups: vec![
            (
                PlatformKey::Literal("win32".to_owned()),
                vec![(0, all), (12, modern.clone())],
            ),
            (PlatformKey::All, vec![(0, modern.clone())]),
            (PlatformKey::Other, vec![(0, modern)]),
        ],
    }
}

fn target(minor: u32, platform: StubTargetPlatform) -> StubTarget {
    StubTarget {
        python_version: (3, minor),
        platform,
    }
}

#[test]
fn roundtrip_preserves_every_field_shape() -> Result<(), BuiltinsIndexError> {
    let artifact = synthetic_artifact();
    let encoded = codec::encode(&artifact, "deadbeef")?;
    let (sha, decoded) = codec::decode(&encoded)?;
    assert_eq!(sha, "deadbeef");
    assert_eq!(decoded.classes(0)?, artifact.default_classes);
    let expected: Vec<&HashMap<String, StubClass>> = artifact
        .groups
        .iter()
        .flat_map(|(_, intervals)| intervals.iter().map(|(_, classes)| classes))
        .collect();
    for (index, classes) in expected.into_iter().enumerate() {
        assert_eq!(&decoded.classes(index + 1)?, classes);
    }
    Ok(())
}

#[test]
fn decoded_keys_select_the_right_variant() -> Result<(), BuiltinsIndexError> {
    let encoded = codec::encode(&synthetic_artifact(), "cafe")?;
    let (_, decoded) = codec::decode(&encoded)?;
    let win32 = || StubTargetPlatform::Concrete("win32".to_owned());
    assert_eq!(decoded.variant_for(None), Some(0));
    for minor in [0_u32, 5, 11] {
        let selected = decoded.variant_for(Some(&target(minor, win32())));
        assert_eq!(selected, Some(1), "win32 3.{minor}");
    }
    for minor in [12_u32, 13, 99] {
        let selected = decoded.variant_for(Some(&target(minor, win32())));
        assert_eq!(selected, Some(2), "win32 3.{minor}");
    }
    let all = decoded.variant_for(Some(&target(13, StubTargetPlatform::All)));
    assert_eq!(all, Some(3), "the All platform class has its own variant");
    let unnamed = StubTargetPlatform::Concrete("haiku".to_owned());
    let other = decoded.variant_for(Some(&target(13, unnamed)));
    assert_eq!(other, Some(4), "an unnamed platform lands in the Other class");
    let four = target(0, win32());
    assert_eq!(
        decoded.variant_for(Some(&StubTarget {
            python_version: (4, 0),
            ..four
        })),
        None
    );
    Ok(())
}

#[test]
fn encoding_is_deterministic_across_map_orderings() -> Result<(), BuiltinsIndexError> {
    let artifact = synthetic_artifact();
    let mut pairs: Vec<(String, StubClass)> = artifact
        .default_classes
        .iter()
        .map(|(name, class)| (name.clone(), class.clone()))
        .collect();
    pairs.sort_by(|left, right| right.0.cmp(&left.0));
    let reversed = Artifact {
        default_classes: pairs.into_iter().collect(),
        groups: artifact.groups.clone(),
    };
    assert_eq!(
        codec::encode(&artifact, "cafe")?,
        codec::encode(&reversed, "cafe")?
    );
    Ok(())
}

#[test]
fn malformed_artifacts_are_rejected_not_panicked() -> Result<(), BuiltinsIndexError> {
    assert!(codec::decode(&[]).is_err(), "empty input must be rejected");
    assert!(
        codec::decode(b"NOTMAGIC").is_err(),
        "wrong magic must be rejected"
    );
    let valid = codec::encode(&synthetic_artifact(), "beef")?;
    for cut in [1, MAGIC.len(), valid.len() / 2, valid.len() - 1] {
        let truncated = valid.get(..cut).unwrap_or(&[]);
        assert!(
            codec::decode(truncated).is_err(),
            "truncation at {cut} must be rejected"
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(
        codec::decode(&trailing).is_err(),
        "trailing bytes must be rejected"
    );
    // Layout walk to the first pooled class's metaclass tag: magic (8) + sha
    // "beef" (4-byte length + 4 bytes) + pool count (4) + blob length (4) +
    // name "Empty" (4 + 5) + bases count (4) = byte 41. An option tag outside
    // {0, 1} is malformed.
    let mut bad_tag = valid;
    if let Some(tag) = bad_tag.get_mut(41) {
        *tag = 9;
    }
    let decoded = codec::decode(&bad_tag).and_then(|(_, artifact)| artifact.classes(0));
    assert!(
        decoded.is_err(),
        "corrupted option tag must be rejected on read"
    );
    Ok(())
}
