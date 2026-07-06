# typeDiagram model - https://typediagram.dev
# Source-of-truth model for stub resolution (STUBRES-ENGINE).
# Implemented by StubResolution / StubSource / StubTier / TypeProvenance in
# crates/basilisk-stubs/src/types.rs — the enum shapes are GENERATED from this
# markup (hand-rolling the ADTs is forbidden); the Rust wrapper only adds derives,
# doc comments and the From/hover_label impls (behaviour, not the DTO).
# CustomTypeshed / StubCustomTypeshed are the step-3 `typeshed-path` override
# requested in issue #271 (the canonical stdlib source); see
# CHECKER-STUB-RESOLUTION-PLAN.md#STUBRESPLAN-TODO.
# pyi_path is a std::path::PathBuf in Rust; modelled as String (opaque) here.
# Regenerate the diagram: typediagram models/stub_resolution.td > docs/models/stub_resolution.svg
# Generate the Rust ADTs:  typediagram --to rust models/stub_resolution.td

alias PathBuf = String

type StubResolution {
  module: String
  source: StubSource
  pyi_path: Option<PathBuf>
  tier: StubTier
}

union StubSource {
  UserStub
  StubPackage
  InlineTyped
  Typeshed
  CustomTypeshed
}

union StubTier {
  Tier1
  Tier2
  Tier3
}

# Checker-side provenance projected from (StubSource, StubTier): what a tracked
# type's information is worth, driving cascade suppression + hover labels.
# StubCustomTypeshed keeps Tier-1 trust but its own identity so a custom
# typeshed (#271) never masquerades as the bundled one.
union TypeProvenance {
  Source
  StubTier1
  StubCustomTypeshed
  StubTier2
  StubTier3
  Untyped
}
