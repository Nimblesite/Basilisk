//! Build metadata for the Shipwright `--version` contract.
//! Implements [CHKARCH-ARCH-BUILD-VERSIONINFO].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-BUILD-VERSIONINFO

fn main() {
    basilisk_buildinfo::emit_version_env();
}
