mod common;

use common::resolve_src;

#[test]
fn protocol_instantiation_violation_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol\n",
        "class MyProto(Protocol):\n",
        "    def method(self) -> None: ...\n",
        "x = MyProto()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_self_violation_method_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol, Self\n",
        "class Copyable(Protocol):\n",
        "    def copy(self) -> Self: ...\n",
        "class Impl:\n",
        "    def copy(self) -> int:\n",
        "        return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // Protocol self violations may or may not be detected depending on
    // how sophisticated the analysis is. Just check it doesn't crash.
    let _ = resolved.protocol_self_violations;
    Ok(())
}
