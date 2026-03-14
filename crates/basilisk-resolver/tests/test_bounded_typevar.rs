mod common;

use common::resolve_src;

#[test]
fn bounded_typevar_detects_invalid_attr_on_str_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.nonexistent_method()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.bounded_typevar_attr_violations.is_empty(),
        "accessing nonexistent attribute on str-bounded TypeVar must produce a violation"
    );
    let v = &resolved.bounded_typevar_attr_violations[0];
    assert_eq!(v.bound_type, "str");
    assert_eq!(v.attr_name, "nonexistent_method");
    Ok(())
}

#[test]
fn bounded_typevar_allows_valid_str_attr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.upper()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.bounded_typevar_attr_violations.is_empty(),
        "accessing valid str attribute must not produce a violation"
    );
    Ok(())
}

#[test]
fn bounded_typevar_detects_invalid_attr_on_int_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: int]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.fake_method()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.bounded_typevar_attr_violations.is_empty(),
        "accessing nonexistent attribute on int-bounded TypeVar must produce a violation"
    );
    assert_eq!(
        resolved.bounded_typevar_attr_violations[0].bound_type,
        "int"
    );
    Ok(())
}

#[test]
fn bounded_typevar_detects_invalid_attr_on_float_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: float]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    assert_eq!(
        resolved.bounded_typevar_attr_violations[0].bound_type,
        "float"
    );
    Ok(())
}

#[test]
fn bounded_typevar_detects_invalid_attr_on_bytes_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: bytes]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    assert_eq!(
        resolved.bounded_typevar_attr_violations[0].bound_type,
        "bytes"
    );
    Ok(())
}

#[test]
fn bounded_typevar_detects_invalid_attr_on_list_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: list]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    assert_eq!(
        resolved.bounded_typevar_attr_violations[0].bound_type,
        "list"
    );
    Ok(())
}

#[test]
fn bounded_typevar_detects_invalid_attr_on_dict_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: dict]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    assert_eq!(
        resolved.bounded_typevar_attr_violations[0].bound_type,
        "dict"
    );
    Ok(())
}

#[test]
fn bounded_typevar_no_violation_for_unknown_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: MyCustomType]:\n",
        "    def process(self, value: T) -> None:\n",
        "        value.whatever()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.bounded_typevar_attr_violations.is_empty(),
        "unknown bound type must not produce violations"
    );
    Ok(())
}

#[test]
fn bounded_typevar_walks_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        if True:\n",
        "            value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        for i in range(10):\n",
        "            value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        while True:\n",
        "            value.nonexistent()\n",
        "            break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        with open('f') as g:\n",
        "            value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        try:\n",
        "            value.nonexistent()\n",
        "        except Exception:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_return_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> str:\n",
        "        return value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_assign_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x = value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_ann_assign_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x: str = value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_binop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x = value.nonexistent() + 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_boolop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x = True or value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_compare_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x = value.nonexistent() == 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_walks_unaryop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, value: T) -> None:\n",
        "        x = not value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_kwonly_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Container[T: str]:\n",
        "    def process(self, *, value: T) -> None:\n",
        "        value.nonexistent()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}
