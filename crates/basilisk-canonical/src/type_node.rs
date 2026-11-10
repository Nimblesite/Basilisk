//! Implements [RESOLV-CANONICAL-RELATION].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! The resolved structure of a type expression.
//!
//! Lowering turns an annotation's `Expr` into a [`TypeNode`] by asking the
//! [`BindingTable`] what each reference denotes — never by reading the
//! characters at the use site. `typing.Optional[int]`, `Opt[int]` under
//! `from typing import Optional as Opt`, and `int | None` all lower to the
//! same node. Anything the bindings cannot resolve lowers to
//! [`TypeNode::Unknown`], on which every relation abstains.

use ruff_python_ast::{self as ast, Expr};

use crate::binding::BindingTable;
use crate::form::TypingForm;

/// A builtin class named by a type expression, resolved by definition site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinClass {
    /// `int`.
    Int,
    /// `float`.
    Float,
    /// `complex`.
    Complex,
    /// `bool`.
    Bool,
    /// `str`.
    Str,
    /// `bytes`.
    Bytes,
    /// `bytearray`.
    Bytearray,
    /// `object`.
    Object,
    /// `list`.
    List,
    /// `dict`.
    Dict,
    /// `set`.
    Set,
    /// `frozenset`.
    Frozenset,
    /// `tuple`.
    Tuple,
    /// `type`.
    Type,
}

/// A value inhabiting a `Literal` type (PEP 586).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LiteralValue {
    /// A boolean literal — distinct from `Literal[1]`/`Literal[0]`.
    Bool(bool),
    /// An integer literal within `i64` range.
    Int(i64),
    /// A string literal.
    Str(Box<str>),
    /// A bytes literal.
    Bytes(Box<[u8]>),
}

impl LiteralValue {
    /// The builtin class this value is an instance of.
    #[must_use]
    pub fn value_class(&self) -> BuiltinClass {
        match self {
            Self::Bool(_) => BuiltinClass::Bool,
            Self::Int(_) => BuiltinClass::Int,
            Self::Str(_) => BuiltinClass::Str,
            Self::Bytes(_) => BuiltinClass::Bytes,
        }
    }
}

/// The resolved structure of a type expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeNode {
    /// `Any` — the gradual type, consistent with everything.
    Any,
    /// `Never` / `NoReturn` — the empty type.
    Never,
    /// `None` / `types.NoneType`.
    NoneType,
    /// `LiteralString` (PEP 675).
    LiteralString,
    /// `...` inside `tuple[T, ...]` or `Callable[..., R]`.
    Ellipsis,
    /// A builtin class, unparameterized.
    Builtin(BuiltinClass),
    /// A single-value `Literal`; multi-value literals lower to a union.
    Literal(LiteralValue),
    /// A union of members, flattened and deduplicated.
    Union(Vec<TypeNode>),
    /// A parameterized type: `base[args…]`.
    Subscript {
        /// The parameterized constructor.
        base: Box<TypeNode>,
        /// The type arguments, in order.
        args: Vec<TypeNode>,
    },
    /// A resolved specification form the relations do not model structurally.
    Form(TypingForm),
    /// Unresolvable — every relation over it abstains.
    Unknown,
}

impl TypeNode {
    /// Lower a type expression to its resolved structure.
    #[must_use]
    pub fn lower(bindings: &BindingTable, expr: &Expr) -> Self {
        match expr {
            Expr::NoneLiteral(_) => Self::NoneType,
            Expr::EllipsisLiteral(_) => Self::Ellipsis,
            Expr::BinOp(op) if matches!(op.op, ast::Operator::BitOr) => union_of(vec![
                Self::lower(bindings, &op.left),
                Self::lower(bindings, &op.right),
            ]),
            Expr::Subscript(sub) => lower_subscript(bindings, sub),
            Expr::Name(_) | Expr::Attribute(_) => lower_reference(bindings, expr),
            _ => Self::Unknown,
        }
    }

    /// The type of a literal *value* expression, for call arguments and
    /// assignment right-hand sides: `3` is `Literal[3]`, `1.5` is `float`.
    #[must_use]
    pub fn of_literal_expr(expr: &Expr) -> Self {
        match expr {
            Expr::NumberLiteral(number) => of_number(&number.value),
            Expr::StringLiteral(string) => {
                Self::Literal(LiteralValue::Str(string.value.to_str().into()))
            }
            Expr::BytesLiteral(bytes) => {
                Self::Literal(LiteralValue::Bytes(bytes.value.bytes().collect()))
            }
            Expr::BooleanLiteral(boolean) => Self::Literal(LiteralValue::Bool(boolean.value)),
            Expr::NoneLiteral(_) => Self::NoneType,
            Expr::UnaryOp(unary) if matches!(unary.op, ast::UnaryOp::USub) => {
                negated_int(&unary.operand)
            }
            _ => Self::Unknown,
        }
    }
}

/// The node for a numeric literal.
fn of_number(number: &ast::Number) -> TypeNode {
    match number {
        ast::Number::Int(value) => value
            .as_i64()
            .map_or(TypeNode::Builtin(BuiltinClass::Int), |value| {
                TypeNode::Literal(LiteralValue::Int(value))
            }),
        ast::Number::Float(_) => TypeNode::Builtin(BuiltinClass::Float),
        ast::Number::Complex { .. } => TypeNode::Builtin(BuiltinClass::Complex),
    }
}

/// The node for a negated integer literal, `-3`.
fn negated_int(operand: &Expr) -> TypeNode {
    let Expr::NumberLiteral(number) = operand else {
        return TypeNode::Unknown;
    };
    let ast::Number::Int(value) = &number.value else {
        return TypeNode::Builtin(BuiltinClass::Float);
    };
    value
        .as_i64()
        .and_then(i64::checked_neg)
        .map_or(TypeNode::Builtin(BuiltinClass::Int), |value| {
            TypeNode::Literal(LiteralValue::Int(value))
        })
}

/// Lower a `Name`/`Attribute` reference through binding resolution.
fn lower_reference(bindings: &BindingTable, expr: &Expr) -> TypeNode {
    bindings
        .form_of_with_builtins(expr)
        .map_or(TypeNode::Unknown, form_to_node)
}

/// The node a resolved specification form denotes when used bare.
fn form_to_node(form: TypingForm) -> TypeNode {
    match form {
        TypingForm::Any => TypeNode::Any,
        TypingForm::Never | TypingForm::NoReturn => TypeNode::Never,
        TypingForm::LiteralString => TypeNode::LiteralString,
        TypingForm::NoneTypeClass => TypeNode::NoneType,
        _ => builtin_class_of(form).map_or(TypeNode::Form(form), TypeNode::Builtin),
    }
}

/// The builtin class a form denotes, unifying the PEP 585 aliases with the
/// classes themselves: `typing.List` IS `list`.
fn builtin_class_of(form: TypingForm) -> Option<BuiltinClass> {
    match form {
        TypingForm::IntClass => Some(BuiltinClass::Int),
        TypingForm::FloatClass => Some(BuiltinClass::Float),
        TypingForm::ComplexClass => Some(BuiltinClass::Complex),
        TypingForm::BoolClass => Some(BuiltinClass::Bool),
        TypingForm::StrClass => Some(BuiltinClass::Str),
        TypingForm::BytesClass => Some(BuiltinClass::Bytes),
        TypingForm::BytearrayClass => Some(BuiltinClass::Bytearray),
        TypingForm::ObjectClass => Some(BuiltinClass::Object),
        TypingForm::ListClass | TypingForm::ListAlias => Some(BuiltinClass::List),
        TypingForm::DictClass | TypingForm::DictAlias => Some(BuiltinClass::Dict),
        TypingForm::SetClass | TypingForm::SetAlias => Some(BuiltinClass::Set),
        TypingForm::FrozensetClass | TypingForm::FrozensetAlias => Some(BuiltinClass::Frozenset),
        TypingForm::TupleClass | TypingForm::TupleAlias => Some(BuiltinClass::Tuple),
        TypingForm::TypeClass | TypingForm::TypeAliasBuiltin => Some(BuiltinClass::Type),
        _ => None,
    }
}

/// Lower a subscripted type expression by what its base denotes.
fn lower_subscript(bindings: &BindingTable, sub: &ast::ExprSubscript) -> TypeNode {
    let Some(form) = bindings.form_of_with_builtins(&sub.value) else {
        return TypeNode::Subscript {
            base: Box::new(TypeNode::Unknown),
            args: slice_args(bindings, &sub.slice),
        };
    };
    match form {
        TypingForm::Optional => union_of(vec![
            TypeNode::lower(bindings, &sub.slice),
            TypeNode::NoneType,
        ]),
        TypingForm::Union => union_of(slice_args(bindings, &sub.slice)),
        TypingForm::Literal => lower_literal(bindings, &sub.slice),
        TypingForm::Annotated => lower_annotated(bindings, &sub.slice),
        form if form.is_annotation_qualifier() => TypeNode::lower(bindings, &sub.slice),
        form => TypeNode::Subscript {
            base: Box::new(form_to_node(form)),
            args: slice_args(bindings, &sub.slice),
        },
    }
}

/// The type arguments of a subscript slice: a tuple's elements, or the single
/// expression itself.
fn slice_args(bindings: &BindingTable, slice: &Expr) -> Vec<TypeNode> {
    match slice {
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(|element| TypeNode::lower(bindings, element))
            .collect(),
        single => vec![TypeNode::lower(bindings, single)],
    }
}

/// Lower `Annotated[T, metadata…]` to `T`: metadata never affects the type
/// (PEP 593).
fn lower_annotated(bindings: &BindingTable, slice: &Expr) -> TypeNode {
    match slice {
        Expr::Tuple(tuple) => tuple
            .elts
            .first()
            .map_or(TypeNode::Unknown, |first| TypeNode::lower(bindings, first)),
        _ => TypeNode::Unknown,
    }
}

/// Lower `Literal[…]` arguments: each value becomes a single-value literal,
/// `None` becomes `NoneType`, and multiple values become their union — the
/// PEP 586 equivalences.
fn lower_literal(bindings: &BindingTable, slice: &Expr) -> TypeNode {
    let members = match slice {
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(|element| lower_literal_member(bindings, element))
            .collect(),
        single => vec![lower_literal_member(bindings, single)],
    };
    union_of(members)
}

/// Lower one `Literal` argument.
fn lower_literal_member(bindings: &BindingTable, expr: &Expr) -> TypeNode {
    match expr {
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::UnaryOp(_) => TypeNode::of_literal_expr(expr),
        // `Literal[Literal[…]]` flattens (PEP 586); enum members and every
        // other reference are outside this layer's model.
        Expr::Subscript(sub) => lower_subscript(bindings, sub),
        _ => TypeNode::Unknown,
    }
}

/// A union of `members`, flattened, deduplicated, and collapsed when single.
fn union_of(members: Vec<TypeNode>) -> TypeNode {
    let mut flat: Vec<TypeNode> = Vec::with_capacity(members.len());
    for member in members {
        match member {
            TypeNode::Union(inner) => {
                for node in inner {
                    if !flat.contains(&node) {
                        flat.push(node);
                    }
                }
            }
            node => {
                if !flat.contains(&node) {
                    flat.push(node);
                }
            }
        }
    }
    match flat.len() {
        0 => TypeNode::Unknown,
        1 => flat.swap_remove(0),
        _ => TypeNode::Union(flat),
    }
}
