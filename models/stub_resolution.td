# typeDiagram model - https://typediagram.dev
# Source-of-truth model for stub resolution (STUBRES-ENGINE).
# Implemented by StubResolution / StubSource / StubTier in
# crates/basilisk-stubs/src/types.rs.
# CustomTypeshed is the step-3 `typeshed-path` override requested in issue #271
# (the canonical stdlib source); see CHECKER-STUB-RESOLUTION-PLAN.md#STUBRESPLAN-TODO.
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
