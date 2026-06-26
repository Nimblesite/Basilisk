// Implements [WEBSITE-SCREENSHOTS-MANIFEST]: the single source of truth for every
// CLI screenshot on the site. See docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.
//
// Each entry pairs the EXACT snippet shown in the docs with the diagnostic code
// that snippet must produce. The generator runs the real `basilisk` binary on the
// snippet and refuses to write the image unless `expect` appears in the output —
// this is the automated form of the "verify the example actually triggers the
// rule" rule, so a checker behaviour change can never silently produce a
// misleading screenshot.
//
// `name` is the output PNG stem (website/src/assets/images/<name>.png) and matches
// the reference used by the docs page (e.g. `e0001` → e0001.png).

// Rule screenshots — the "# Error" snippet from docs/rules/*.md, crafted so that
// exactly the documented rule fires (e.g. e0001 keeps `-> str` so only E0001,
// not E0002, is reported).
const RULE_SHOTS = [
  {
    name: "e0001",
    expect: "BSK-E0001",
    code: `def process(data) -> str:
    return data.upper()
`,
  },
  {
    name: "e0002",
    expect: "BSK-E0002",
    code: `def get_user(user_id: int):
    return {"id": user_id}
`,
  },
  {
    name: "e0003",
    expect: "BSK-E0003",
    code: `data = []
`,
  },
  {
    name: "e0004",
    expect: "BSK-E0004",
    code: `def log(*args, **kwargs) -> None:
    print(args, kwargs)
`,
  },
  {
    name: "e0005",
    expect: "BSK-E0005",
    code: `class Registry:
    entries = []
`,
  },
  {
    name: "e0010",
    expect: "BSK-E0010",
    code: `from legacy_module import process_data
`,
  },
  {
    name: "e0011",
    expect: "BSK-W0014",
    code: `from typing import Any


def handle(data: Any) -> bool:
    return True
`,
  },
  {
    name: "e0012",
    expect: "BSK-E0012",
    code: `def greet(name: str) -> str:
    return f"Hello, {name}"


greet(42)
`,
  },
  {
    name: "e0013",
    expect: "BSK-E0013",
    code: `def get_count() -> int:
    return "many"
`,
  },
  {
    name: "e0014",
    expect: "BSK-E0014",
    code: `count: int = "zero"
`,
  },
  {
    name: "e0015",
    expect: "BSK-E0015",
    code: `x: dict[str] = {}
`,
  },
  {
    name: "e0016",
    expect: "BSK-E0016",
    code: `from typing import override


class Base:
    def process(self, data: str) -> str:
        return data


class Child(Base):
    @override
    def process(self, data: int) -> str:
        return str(data)
`,
  },
  {
    name: "e0018",
    expect: "BSK-E0018",
    code: `def f() -> int:
    return missing_local
`,
  },
  {
    name: "e0019",
    expect: "BSK-E0019",
    code: `def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result
`,
  },
  {
    name: "e0025",
    expect: "BSK-E0025",
    code: `class Base:
    def process(self) -> str:
        return "base"


class Child(Base):
    def process(self) -> str:
        return "child"
`,
  },
  {
    name: "e0017",
    expect: "BSK-E0017",
    code: `class Base:
    x: int


class Child(Base):
    x: str
`,
  },
  {
    name: "e0020",
    expect: "BSK-E0020",
    code: `from typing import overload


@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...
`,
  },
  {
    name: "e0023",
    expect: "BSK-E0023",
    code: `def classify(x: int | str) -> str:
    match x:
        case int():
            return "number"
`,
  },
  {
    name: "e0026",
    expect: "BSK-E0026",
    code: `from typing import TypeVar

T = TypeVar("T", int)
`,
  },
  {
    name: "e0027",
    expect: "BSK-E0027",
    code: `from typing import Generic, TypeVar

T = TypeVar("T")


class Box(Generic[T, T]):
    ...
`,
  },
  {
    name: "e0029",
    expect: "BSK-E0029",
    code: `from typing import TypedDict


class Movie(TypedDict):
    title: str

    def play(self) -> None:
        ...
`,
  },
  {
    name: "e0031",
    expect: "BSK-E0031",
    code: `from typing import cast

x = cast(int)
`,
  },
  {
    name: "e0033",
    expect: "BSK-E0033",
    code: `reveal_type()
`,
  },
  {
    name: "e0040",
    expect: "BSK-E0040",
    code: `from enum import Enum


class Base(Enum):
    A = 1


class Sub(Base):
    B = 2
`,
  },
  {
    name: "e0041",
    expect: "BSK-E0041",
    code: `def add(x: int, y: int) -> int:
    return x + y


add(1)
`,
  },
  {
    name: "e0099",
    expect: "BSK-E0099",
    code: `from typing import Protocol


class P(Protocol):
    def f(self) -> None: ...


P()
`,
  },
  {
    name: "e0115",
    expect: "BSK-E0115",
    code: `from warnings import deprecated


@deprecated("use bar")
def foo() -> None: ...


foo()
`,
  },
];

// Homepage before/after demo. `bad.py` must report exactly six errors and
// `good.py` must be clean — these mirror the source shown in website/src/index.njk.
const HOME_SHOTS = [
  {
    name: "cli-demo",
    file: "bad.py",
    expect: "Found 6 diagnostics",
    code: `def process(data):
    return data.upper()

class User:
    def __init__(self, name, age):
        self.name = name
        self.age  = age

    def greet(self):
        return f"Hello, {self.name}"
`,
  },
  {
    name: "cli-clean",
    file: "good.py",
    expect: "No issues found",
    code: `def process(data: str) -> str:
    return data.upper()


class User:
    name: str
    age: int

    def __init__(self, name: str, age: int) -> None:
        self.name = name
        self.age  = age

    def greet(self) -> str:
        return f"Hello, {self.name}"
`,
  },
];

// A rule shot's source file is named after the image stem (e0001 → e0001.py); a
// home shot carries its own filename so the prompt reads `basilisk check bad.py`.
export const SHOTS = [
  ...RULE_SHOTS.map((shot) => ({ ...shot, file: `${shot.name}.py` })),
  ...HOME_SHOTS,
];
