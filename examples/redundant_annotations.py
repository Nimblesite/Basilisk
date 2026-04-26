# Redundant type annotations — Basilisk infers these automatically.
#
# W0050 fires when the annotation adds no information beyond what inference provides.
# E0005 does NOT fire when a subclass overrides a parent's annotated attribute.

# ---------------------------------------------------------------------------
# Module-level: scalar literals are always inferrable
# ---------------------------------------------------------------------------

count: int = 42  # W0050 — obviously int
name: str = "hello"  # W0050 — obviously str
rate: float = 3.14  # W0050 — obviously float
enabled: bool = True  # W0050 — obviously bool
disabled: bool = False  # W0050 — obviously bool
header: bytes = b"\x00\xff"  # W0050 — obviously bytes
nothing: None = None  # W0050 — obviously None

# Edge cases: zero/empty values
zero: int = 0  # W0050 — still obviously int
empty: str = ""  # W0050 — still obviously str
zero_f: float = 0.0  # W0050 — still obviously float

# ---------------------------------------------------------------------------
# Module-level: annotations that ADD information (no W0050)
# ---------------------------------------------------------------------------

widened: float = 42  # NO warning — int widened to float
items: list[int] = [1, 2, 3]  # NO warning — collection type is useful
pairs: dict[str, int] = {"a": 1}  # NO warning — collection type is useful
nums: set[int] = {1, 2, 3}  # NO warning — collection type is useful
coords: tuple[int, int] = (1, 2)  # NO warning — collection type is useful


# ---------------------------------------------------------------------------
# Class attributes: same rules apply
# ---------------------------------------------------------------------------


class Settings:
    retries: int = 3  # W0050 — redundant
    label: str = "default"  # W0050 — redundant
    threshold: float = 0.5  # W0050 — redundant
    verbose: bool = True  # W0050 — redundant
    magic: bytes = b"\x00"  # W0050 — redundant
    nothing: None = None  # W0050 — redundant


# ---------------------------------------------------------------------------
# Subclass overrides: inherited annotation satisfies E0005
# ---------------------------------------------------------------------------


class BaseRoute:
    path: str = "/"
    method: str = "GET"
    auth_required: bool = False
    priority: int = 0
    timeout: float = 30.0


class AuthenticatedRoute(BaseRoute):
    auth_required = True  # NO E0005 — inherits bool from BaseRoute


class AdminRoute(AuthenticatedRoute):
    priority = 100  # NO E0005 — inherits int from BaseRoute (grandparent)
    path = "/admin"  # NO E0005 — inherits str from BaseRoute (grandparent)


class ApiRoute(AuthenticatedRoute):
    method = "POST"  # NO E0005 — inherits str from BaseRoute
    timeout = 60.0  # NO E0005 — inherits float from BaseRoute
    path = "/api"  # NO E0005 — inherits str from BaseRoute


# ---------------------------------------------------------------------------
# Deep inheritance: annotation flows through the whole chain
# ---------------------------------------------------------------------------


class A:
    tag: str = "a"


class B(A):
    tag = "b"  # NO E0005 — inherits from A


class C(B):
    tag = "c"  # NO E0005 — inherits from A through B


class D(C):
    tag = "d"  # NO E0005 — inherits from A through B -> C


# ---------------------------------------------------------------------------
# Multiple inheritance: annotation from ANY base suffices
# ---------------------------------------------------------------------------


class PriorityMixin:
    priority: int = 0


class Serializable:
    pass


class PrioritizedItem(PriorityMixin, Serializable):
    priority = 10  # NO E0005 — inherits from PriorityMixin


class WeightMixin:
    weight: float = 1.0


class WeightedItem(Serializable, WeightMixin):
    weight = 5.0  # NO E0005 — inherits from WeightMixin (second base)


# ---------------------------------------------------------------------------
# Diamond inheritance: reachable through either path
# ---------------------------------------------------------------------------


class Root:
    value: int = 0


class Left(Root):
    pass


class Right(Root):
    pass


class Diamond(Left, Right):
    value = 42  # NO E0005 — reachable through Left -> Root or Right -> Root


# ---------------------------------------------------------------------------
# Sibling classes independently overriding
# ---------------------------------------------------------------------------


class Animal:
    sound = "..."
    legs = 4


class Dog(Animal):
    sound = "woof"  # NO E0005


class Cat(Animal):
    sound = "meow"  # NO E0005


class Snake(Animal):
    legs = 0  # NO E0005
    sound = "hiss"  # NO E0005


# ---------------------------------------------------------------------------
# Annotation-only parent (no default): child still inherits the type
# ---------------------------------------------------------------------------


class AbstractHandler:
    name: str


class ConcreteHandler(AbstractHandler):
    name = "default"  # NO E0005 — parent declared `name: str`


# ---------------------------------------------------------------------------
# Config pattern: production/staging overrides
# ---------------------------------------------------------------------------


class DatabaseConfig:
    host = "localhost"
    port = 5432
    pool_size = 10
    ssl = False


class ProductionDB(DatabaseConfig):
    host = "db.prod.internal"
    port = 5433
    ssl = True
    pool_size = 50


class StagingDB(DatabaseConfig):
    host = "db.staging.internal"
    pool_size = 5


# ---------------------------------------------------------------------------
# Scalar literals in standalone classes — type is inferrable, NO E0005
# ---------------------------------------------------------------------------


class Standalone:
    value = 42  # NO E0005 — scalar literal, type is trivially `int`


class UnannotatedParent:
    raw = 99  # NO E0005 — scalar literal, type is trivially `int`


class ChildOfUnannotated(UnannotatedParent):
    raw = 100  # NO E0005 — scalar literal, type is trivially `int`


class UnrelatedToBaseRoute:
    path = "/unrelated"  # NO E0005 — scalar literal, type is trivially `str`


# ---------------------------------------------------------------------------
# Function parameters: annotation is required (no W0050 — params need types)
# ---------------------------------------------------------------------------


def greet(name: str, count: int = 1) -> str:  # NO W0050 — params need annotations
    return name * count


# ---------------------------------------------------------------------------
# Function return types: redundant when inferrable from body
# ---------------------------------------------------------------------------


def get_count() -> int:  # W0050 — return type inferrable from `return 42`
    return 42


def get_name() -> str:  # W0050 — return type inferrable from `return "hello"`
    return "hello"


def get_flag() -> bool:  # W0050 — return type inferrable from `return True`
    return True


def get_rate() -> float:  # W0050 — return type inferrable from `return 3.14`
    return 3.14


def get_data() -> bytes:  # W0050 — return type inferrable from `return b"\x00"`
    return b"\x00"


def get_nothing() -> None:  # W0050 — return type inferrable from `return None`
    return None


def implicit_none() -> None:  # W0050 — no return statement implies None
    pass


# ---------------------------------------------------------------------------
# Function return types that ADD information (no W0050)
# ---------------------------------------------------------------------------


def get_items() -> list[int]:  # NO W0050 — collection type adds info
    return [1, 2, 3]


def get_mapping() -> dict[str, int]:  # NO W0050 — collection type adds info
    return {"a": 1}


def widen_return() -> float:  # NO W0050 — widening int to float
    return 42


def conditional_return(flag: bool) -> str:  # NO W0050 — multiple return paths
    if flag:
        return "yes"
    return "no"


# ---------------------------------------------------------------------------
# Local variables: redundant annotations
# ---------------------------------------------------------------------------


def local_scalars() -> None:
    x: int = 10  # W0050 — obviously int
    y: str = "world"  # W0050 — obviously str
    z: float = 2.71  # W0050 — obviously float
    flag: bool = False  # W0050 — obviously bool
    raw: bytes = b"\xff"  # W0050 — obviously bytes
    nope: None = None  # W0050 — obviously None
    _ = (x, y, z, flag, raw, nope)


def local_non_redundant() -> None:
    items: list[int] = [1, 2]  # NO W0050 — collection type adds info
    widened: float = 0  # NO W0050 — int widened to float
    mapping: dict[str, bool] = {}  # NO W0050 — empty collection needs type
    _ = (items, widened, mapping)


# ---------------------------------------------------------------------------
# For-loop variables: redundant annotations
# ---------------------------------------------------------------------------


def loop_annotations() -> None:
    total: int = 0  # W0050 — obviously int
    for i in range(10):
        total += i
    _ = total


# ---------------------------------------------------------------------------
# Comprehension targets captured into annotated variables
# ---------------------------------------------------------------------------


def comprehension_annotations() -> None:
    squares: list[int] = [x * x for x in range(5)]  # NO W0050 — list[int] adds info
    names: list[str] = [s.upper() for s in ["a", "b"]]  # NO W0050 — list[str] adds info
    _ = (squares, names)


# ---------------------------------------------------------------------------
# Lambda: return annotation not possible, but assignment annotation
# ---------------------------------------------------------------------------

double = 2  # NO E0003 — scalar literal, type is trivially `int`
fn = lambda x: x * 2  # NO E0003 — not an unresolvable expression


# ---------------------------------------------------------------------------
# Property: redundant return annotations
# ---------------------------------------------------------------------------


class Circle:
    def __init__(self, radius: float) -> None:  # W0050 — __init__ always returns None
        self._radius = radius

    @property
    def radius(self) -> float:  # NO W0050 — property return types are documentation
        return self._radius

    @property
    def area(self) -> float:  # NO W0050 — computed, annotation documents interface
        return 3.14159 * self._radius**2

    @property
    def name(self) -> str:  # W0050 — trivially returns a literal
        return "circle"

    @property
    def is_unit(self) -> bool:  # W0050 — trivially returns a comparison
        return self._radius == 1.0


# ---------------------------------------------------------------------------
# __init__ and __new__: always return None / cls (redundant)
# ---------------------------------------------------------------------------


class Widget:
    def __init__(self) -> None:  # W0050 — __init__ always returns None
        self.value = 0

    def __repr__(self) -> str:  # W0050 — inferrable from return f"..."
        return f"Widget({self.value})"

    def __str__(self) -> str:  # W0050 — inferrable from return "..."
        return "widget"

    def __len__(self) -> int:  # W0050 — inferrable from return <int>
        return self.value

    def __bool__(self) -> bool:  # W0050 — inferrable from return True/False
        return self.value > 0


# ---------------------------------------------------------------------------
# Staticmethod and classmethod
# ---------------------------------------------------------------------------


class Factory:
    @staticmethod
    def create_default() -> int:  # W0050 — inferrable from `return 0`
        return 0

    @classmethod
    def from_string(
        cls, text: str
    ) -> "Factory":  # NO W0050 — cls return is not inferrable
        return cls()


# ---------------------------------------------------------------------------
# Nested functions: same rules apply
# ---------------------------------------------------------------------------


def outer() -> None:
    def inner_redundant() -> int:  # W0050 — inferrable
        return 99

    def inner_needed() -> list[int]:  # NO W0050 — collection type adds info
        return [1, 2, 3]

    x: int = inner_redundant()  # W0050 — return type known to be int
    y = inner_needed()
    _ = (x, y)


# ---------------------------------------------------------------------------
# Walrus operator (:=): annotated target
# ---------------------------------------------------------------------------


def walrus_examples() -> None:
    if (n := 10) > 5:  # NO W0050 — walrus can't carry annotation
        _ = n


# ---------------------------------------------------------------------------
# Type alias assignments: NOT redundant (these define types, not values)
# ---------------------------------------------------------------------------

from typing import TypeAlias

Vector: TypeAlias = list[float]  # NO W0050 — type alias definition
Matrix: TypeAlias = list[list[float]]  # NO W0050 — type alias definition


# ---------------------------------------------------------------------------
# Annotated but no initializer (declaration-only): NOT redundant
# ---------------------------------------------------------------------------


class DeclarationOnly:
    name: str  # NO W0050 — no value, annotation is the declaration
    age: int  # NO W0050 — no value, annotation is the declaration


# ---------------------------------------------------------------------------
# Augmented assignment: annotation on first use, then augmented
# ---------------------------------------------------------------------------


def augmented_assign() -> None:
    total: int = 0  # W0050 — obviously int
    total += 10
    _ = total


# ---------------------------------------------------------------------------
# Global/nonlocal: annotation at module level, used in function
# ---------------------------------------------------------------------------

_counter: int = 0  # W0050 — obviously int


def increment() -> None:
    global _counter
    _counter += 1


# ---------------------------------------------------------------------------
# Dataclass-style: fields with explicit types
# ---------------------------------------------------------------------------

from dataclasses import dataclass


@dataclass
class Point:
    x: float  # NO W0050 — dataclass field, annotation required
    y: float  # NO W0050 — dataclass field, annotation required


@dataclass
class LabeledPoint:
    x: float  # NO W0050 — dataclass field, annotation required
    y: float  # NO W0050 — dataclass field, annotation required
    label: str = "origin"  # NO W0050 — dataclass field, annotation required for default


# ---------------------------------------------------------------------------
# NamedTuple: annotations are part of the structure definition
# ---------------------------------------------------------------------------

from typing import NamedTuple


class Coordinate(NamedTuple):
    x: float  # NO W0050 — NamedTuple field, annotation required
    y: float  # NO W0050 — NamedTuple field, annotation required
    label: str = "point"  # NO W0050 — NamedTuple field, annotation required


# ---------------------------------------------------------------------------
# TypedDict: annotations are the definition
# ---------------------------------------------------------------------------

from typing import TypedDict


class UserDict(TypedDict):
    name: str  # NO W0050 — TypedDict field, annotation IS the definition
    age: int  # NO W0050 — TypedDict field, annotation IS the definition


# ---------------------------------------------------------------------------
# Constructor calls: annotation redundant when type matches constructor
# ---------------------------------------------------------------------------


def constructor_annotations() -> None:
    x: int = int(42)  # W0050 — int() returns int
    y: str = str("hello")  # W0050 — str() returns str
    z: float = float(1.0)  # W0050 — float() returns float
    b: bool = bool(True)  # W0050 — bool() returns bool
    r: bytes = bytes(b"")  # W0050 — bytes() returns bytes
    lst: list = list()  # W0050 — list() returns list
    dct: dict = dict()  # W0050 — dict() returns dict
    st: set = set()  # W0050 — set() returns set
    _ = (x, y, z, b, r, lst, dct, st)


def constructor_non_redundant() -> None:
    items: list[int] = list()  # NO W0050 — parameterized type adds info
    mapping: dict[str, int] = dict()  # NO W0050 — parameterized type adds info
    _ = (items, mapping)


# ---------------------------------------------------------------------------
# Cast and assertion patterns
# ---------------------------------------------------------------------------

from typing import cast


def cast_patterns() -> None:
    x: int = cast(int, some_value())  # NO W0050 — cast is explicit intent
    _ = x


def some_value() -> object:
    return 42


# ---------------------------------------------------------------------------
# Multiple assignment targets
# ---------------------------------------------------------------------------


def multi_assign() -> None:
    b = 10
    a: int = b  # W0050 — b is already int
    _ = (a, b)


# ---------------------------------------------------------------------------
# String literal types (forward references): NOT redundant
# ---------------------------------------------------------------------------


class Node:
    def next(self) -> "Node":  # NO W0050 — forward reference, not inferrable
        return Node()


# ---------------------------------------------------------------------------
# Union types: NOT redundant
# ---------------------------------------------------------------------------

from typing import Union, Optional

maybe_int: Optional[int] = None  # NO W0050 — Optional adds info beyond None
either: Union[int, str] = 42  # NO W0050 — Union adds info beyond int


# ---------------------------------------------------------------------------
# Final: annotation may be redundant but Final qualifier is not
# ---------------------------------------------------------------------------

from typing import Final

MAX_SIZE: Final[int] = 100  # W0050 — int is redundant (Final alone suffices)
MAX_NAME: Final = "limit"  # NO W0050 — no redundant type, just Final


# ---------------------------------------------------------------------------
# Callable annotations
# ---------------------------------------------------------------------------

from typing import Callable


def apply_func(
    func: Callable[[int], int], value: int
) -> int:  # NO W0050 — Callable needed
    return func(value)


# ---------------------------------------------------------------------------
# Async functions: same rules apply
# ---------------------------------------------------------------------------

import asyncio


async def async_redundant() -> int:  # W0050 — inferrable from `return 42`
    return 42


async def async_needed() -> list[int]:  # NO W0050 — collection type adds info
    return [1, 2, 3]


async def async_none() -> None:  # W0050 — async with no return implies None
    await asyncio.sleep(0)


# ---------------------------------------------------------------------------
# Generator annotations
# ---------------------------------------------------------------------------

from typing import Generator, Iterator


def gen_needed() -> Generator[int, None, None]:  # NO W0050 — Generator type adds info
    yield 1
    yield 2


def iter_needed() -> Iterator[str]:  # NO W0050 — Iterator type adds info
    yield "a"
    yield "b"


# ---------------------------------------------------------------------------
# Context managers
# ---------------------------------------------------------------------------

from contextlib import contextmanager


@contextmanager
def managed_resource() -> Generator[
    str, None, None
]:  # NO W0050 — Generator type needed
    yield "resource"


# ---------------------------------------------------------------------------
# Overloaded functions: annotations are required
# ---------------------------------------------------------------------------

from typing import overload


@overload
def process(x: int) -> int:  # NO W0050 — overload signatures required
    ...
@overload
def process(x: str) -> str:  # NO W0050 — overload signatures required
    ...
def process(x: int | str) -> int | str:
    return x


# ---------------------------------------------------------------------------
# Protocol: annotations define the interface
# ---------------------------------------------------------------------------

from typing import Protocol


class Drawable(Protocol):
    def draw(self) -> None:  # NO W0050 — Protocol method signature
        ...

    x: int  # NO W0050 — Protocol attribute declaration


# ---------------------------------------------------------------------------
# Abstract methods: annotations define the contract
# ---------------------------------------------------------------------------

from abc import ABC, abstractmethod


class Shape(ABC):
    @abstractmethod
    def area(self) -> float:  # NO W0050 — abstract method contract
        ...

    @abstractmethod
    def perimeter(self) -> float:  # NO W0050 — abstract method contract
        ...
