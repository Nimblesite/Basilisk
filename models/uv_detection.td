# typeDiagram model - https://typediagram.dev
# Source-of-truth model for uv-project detection (LSPUV-DETECTION-RESULT).
# Implemented by UvProjectInfo in crates/basilisk-uv/src/detect.rs
# (root is a PathBuf there; modelled as String here since paths are language-neutral).
# Regenerate the diagram: typediagram models/uv_detection.td > docs/models/uv_detection.svg

type UvProjectInfo {
  root: String
  has_lockfile: Bool
  has_tool_uv: Bool
  uv_managed_venv: Bool
  has_python_version: Bool
}
