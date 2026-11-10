#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Apply semantics-preserving transformations to typing conformance tests.

Rename eligible typing-related imports and adjust whitespace without changing
comments, line numbers, or expected-error markers.

Usage:

    uv run --script mutate_typing_conformance.py /path/to/python-typing
"""

from __future__ import annotations

import argparse
import ast
import builtins
import collections
import copy
from dataclasses import dataclass
import io
import keyword
from pathlib import Path
import re
import sys
import tokenize


DEFAULT_IMPORT_MODULES = (
    "typing",
    "typing_extensions",
    "dataclasses",
    "enum",
    "collections.abc",
)
DYNAMIC_NAMESPACE_FUNCTIONS = frozenset(
    {"dir", "eval", "exec", "globals", "locals", "vars"}
)


@dataclass(frozen=True, slots=True)
class Candidate:
    family: str
    label: str
    source: str
    references: int = 0
    ast_identical: bool = False


@dataclass(frozen=True, slots=True)
class SourceFile:
    source: str
    encoding: str


def read_source(path: Path) -> SourceFile:
    content = path.read_bytes()
    encoding, _ = tokenize.detect_encoding(io.BytesIO(content).readline)
    source = content.decode(encoding)
    if source.encode(encoding) != content:
        raise ValueError(
            f"Cannot round-trip {path} with its declared {encoding} encoding"
        )
    return SourceFile(source, encoding)


def comment_signature(source: str) -> list[tuple[int, str]]:
    return [
        (token.start[0], token.string)
        for token in tokenize.generate_tokens(io.StringIO(source).readline)
        if token.type == tokenize.COMMENT
    ]


def validate_candidate(source: str, candidate: Candidate) -> bool:
    if source == candidate.source or source.count("\n") != candidate.source.count("\n"):
        return False
    try:
        original_tree = ast.parse(source)
        candidate_tree = ast.parse(candidate.source)
        if comment_signature(source) != comment_signature(candidate.source):
            return False
        if candidate.ast_identical:
            return ast.dump(original_tree, include_attributes=False) == ast.dump(
                candidate_tree, include_attributes=False
            )
        return True
    except (SyntaxError, tokenize.TokenError, UnicodeError, ValueError):
        return False


def validates_alpha_renaming(
    source: str, candidate: Candidate, original_alias: ast.alias, old: str, new: str
) -> bool:
    """Undo one renamed binding and require the original and candidate ASTs to match."""
    original_tree = ast.parse(source)
    candidate_tree = copy.deepcopy(ast.parse(candidate.source))
    aliases = [
        node
        for node in ast.walk(candidate_tree)
        if isinstance(node, ast.alias)
        and node.name == original_alias.name
        and node.asname == new
        and node.lineno == original_alias.lineno
        and node.col_offset == original_alias.col_offset
    ]
    if len(aliases) != 1:
        return False
    aliases[0].asname = original_alias.asname
    for node in ast.walk(candidate_tree):
        if isinstance(node, ast.Name) and node.id == new:
            node.id = old
    return ast.dump(original_tree, include_attributes=False) == ast.dump(
        candidate_tree, include_attributes=False
    )


def ast_position(source: str, line_number: int, byte_column: int) -> tuple[int, int]:
    """Convert Python AST UTF-8 byte offsets to tokenizer character offsets."""
    line = source.splitlines(keepends=True)[line_number - 1]
    character_column = len(line.encode("utf-8")[:byte_column].decode("utf-8"))
    return line_number, character_column


def docstring_positions(source: str, tree: ast.AST) -> set[tuple[int, int]]:
    positions = set()
    for node in ast.walk(tree):
        if not isinstance(
            node, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            continue
        if node.body and isinstance(node.body[0], ast.Expr):
            expression = node.body[0].value
            if isinstance(expression, ast.Constant) and isinstance(
                expression.value, str
            ):
                positions.add(
                    ast_position(source, expression.lineno, expression.col_offset)
                )
    return positions


def pattern_contains_name(text: str, name: str) -> bool:
    return re.search(rf"(?<!\w){re.escape(name)}(?!\w)", text) is not None


def namespace_is_dynamic(tree: ast.AST) -> bool:
    return any(
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id in DYNAMIC_NAMESPACE_FUNCTIONS
        for node in ast.walk(tree)
    )


def other_binding_exists(tree: ast.AST, target: ast.alias, name: str) -> bool:
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Name)
            and node.id == name
            and isinstance(node.ctx, (ast.Store, ast.Del))
        ):
            return True
        if isinstance(node, ast.arg) and node.arg == name:
            return True
        if (
            isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name == name
        ):
            return True
        if isinstance(node, (ast.Global, ast.Nonlocal)) and name in node.names:
            return True
        if isinstance(node, ast.ExceptHandler) and node.name == name:
            return True
        if isinstance(node, (ast.MatchAs, ast.MatchStar)) and node.name == name:
            return True
        if isinstance(node, ast.MatchMapping) and node.rest == name:
            return True
        if isinstance(node, (ast.TypeVar, ast.TypeVarTuple, ast.ParamSpec)):
            if node.name == name:
                return True
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            for imported in node.names:
                exposed = imported.asname or imported.name.split(".")[-1]
                if imported is not target and exposed == name:
                    return True
    return False


def contains_name(node: ast.AST, name: str) -> bool:
    return any(
        isinstance(item, ast.Name) and item.id == name for item in ast.walk(node)
    )


def safe_import_role(tree: ast.AST, name: str) -> bool:
    """Avoid imports whose spelling can drive checker-specific static semantics."""
    nodes = list(ast.walk(tree))
    if any(
        contains_name(node.test, name)
        for node in nodes
        if isinstance(node, (ast.If, ast.While, ast.IfExp))
    ):
        return False
    decorator_nodes = [
        decorator
        for node in nodes
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef))
        for decorator in node.decorator_list
        if contains_name(decorator, name)
    ]
    subscript_nodes = [
        node
        for node in nodes
        if isinstance(node, ast.Subscript)
        and isinstance(node.value, ast.Name)
        and node.value.id == name
    ]
    class_base = any(
        contains_name(base, name)
        for node in nodes
        if isinstance(node, ast.ClassDef)
        for base in node.bases
    )
    call_callee = any(
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id == name
        for node in nodes
    )
    if class_base and call_callee:
        return False

    sensitive_decorator = any(
        isinstance(node, ast.Call)
        and any(isinstance(argument, ast.Constant) for argument in node.args)
        for node in decorator_nodes
    ) or (bool(decorator_nodes) and call_callee)
    if sensitive_decorator:
        return False
    if any(
        isinstance(value, ast.Constant) and isinstance(value.value, str)
        for node in subscript_nodes
        for value in ast.walk(node.slice)
    ):
        return False
    if any(
        isinstance(node, ast.AnnAssign)
        and contains_name(node.annotation, name)
        and isinstance(node.value, ast.Constant)
        and isinstance(node.value.value, str)
        for node in nodes
    ):
        return False
    return True


def import_candidates(
    source: str,
    tree: ast.Module,
) -> list[Candidate]:
    if namespace_is_dynamic(tree):
        return []
    if any(
        isinstance(node, ast.ImportFrom)
        and any(alias.name == "*" for alias in node.names)
        for node in ast.walk(tree)
    ):
        return []

    tokens = list(tokenize.generate_tokens(io.StringIO(source).readline))
    identifiers = {token.string for token in tokens if token.type == tokenize.NAME}
    docs = docstring_positions(source, tree)
    middle_type = getattr(tokenize, "FSTRING_MIDDLE", -1)
    literal_strings = [
        token.string
        for token in tokens
        if token.type in (tokenize.STRING, middle_type) and token.start not in docs
    ]
    meaningful_comments = [
        token.string
        for token in tokens
        if token.type == tokenize.COMMENT and re.match(r"#\s*type\s*:", token.string)
    ]

    result = []
    for node in tree.body:
        if not isinstance(node, ast.ImportFrom) or not node.module:
            continue
        if node.module not in DEFAULT_IMPORT_MODULES:
            continue
        for alias in node.names:
            old = alias.asname or alias.name
            new = f"Audit{alias.name}"
            if (
                alias.name == "*"
                or old in vars(builtins)
                or old.startswith("__")
                or old == new
                or new in identifiers
                or keyword.iskeyword(new)
                or other_binding_exists(tree, alias, old)
                or not safe_import_role(tree, old)
                or any(pattern_contains_name(text, old) for text in literal_strings)
                or any(pattern_contains_name(text, old) for text in meaningful_comments)
            ):
                continue

            old_positions = {
                ast_position(source, item.lineno, item.col_offset)
                for item in ast.walk(tree)
                if isinstance(item, ast.Name) and item.id == old
            }
            if not old_positions:
                continue

            import_position = ast_position(source, alias.lineno, alias.col_offset)
            alias_position = None
            if alias.asname:
                for index, token in enumerate(tokens):
                    if token.start == import_position:
                        for following in tokens[index + 1 : index + 5]:
                            if (
                                following.type == tokenize.NAME
                                and following.string == old
                            ):
                                alias_position = following.start
                                break
                        break
                if alias_position is None:
                    continue

            changed = []
            for token in tokens:
                if token.start == import_position and alias.asname is None:
                    token = token._replace(string=f"{alias.name} as {new}")
                elif token.start == alias_position or token.start in old_positions:
                    token = token._replace(string=new)
                changed.append(token)

            candidate = Candidate(
                "aliases",
                f"alias_{node.module}_{alias.name}",
                tokenize.untokenize(changed),
                references=len(old_positions),
            )
            if validate_candidate(source, candidate) and validates_alpha_renaming(
                source, candidate, alias, old, new
            ):
                result.append(candidate)
    return result


def whitespace_candidate(source: str, flavor: str) -> Candidate | None:
    tokens = list(tokenize.generate_tokens(io.StringIO(source).readline))
    changed = []
    for index, token in enumerate(tokens):
        following = tokens[index + 1] if index + 1 < len(tokens) else None
        if (
            flavor == "subscript_spacing"
            and token.type == tokenize.NAME
            and following
            and following.string == "["
        ):
            token = token._replace(string=token.string + " ")
        elif (
            flavor == "call_spacing"
            and token.type == tokenize.NAME
            and following
            and following.string == "("
        ):
            token = token._replace(string=token.string + " ")
        elif flavor == "square_spacing" and token.string == "[":
            token = token._replace(string="[ ")
        elif flavor == "square_spacing" and token.string == "]":
            token = token._replace(string=" ]")
        elif flavor == "comma_spacing" and token.string == ",":
            token = token._replace(string=" , ")
        elif flavor == "dot_spacing" and token.string == ".":
            token = token._replace(string=" . ")
        elif flavor == "equals_spacing" and token.string == "=":
            token = token._replace(string=" = ")
        changed.append(token)
    candidate = Candidate(
        "formatting",
        f"format_{flavor}",
        tokenize.untokenize(changed),
        ast_identical=True,
    )
    return candidate if validate_candidate(source, candidate) else None


def mutate_source(source: str) -> tuple[str, list[Candidate]]:
    current = source
    applied: list[Candidate] = []
    while candidates := import_candidates(current, ast.parse(current)):
        selected = candidates[0]
        current = selected.source
        applied.append(selected)
    for flavor in (
        "subscript_spacing",
        "call_spacing",
        "square_spacing",
        "comma_spacing",
        "dot_spacing",
        "equals_spacing",
    ):
        candidate = whitespace_candidate(current, flavor)
        if candidate:
            current = candidate.source
            applied.append(candidate)
    return current, applied


def locate_tests(checkout: Path) -> Path:
    checkout = checkout.expanduser().resolve()
    for candidate in (checkout / "conformance" / "tests", checkout / "tests", checkout):
        if candidate.is_dir() and (candidate / "ty.toml").exists():
            return candidate
    raise SystemExit(f"Could not locate conformance/tests under {checkout}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "checkout",
        type=Path,
        help="python/typing checkout or conformance/tests directory",
    )
    return parser


def main() -> None:
    arguments = build_parser().parse_args()
    tests = locate_tests(arguments.checkout)

    fixtures = sorted(
        path
        for pattern in ("*.py", "*.pyi")
        for path in tests.glob(pattern)
        if not path.name.startswith("_")
    )
    modified = 0
    errors = 0
    mutation_counts: collections.Counter[str] = collections.Counter()
    for path in fixtures:
        try:
            original = read_source(path)
            mutated, mutations = mutate_source(original.source)
            if mutations:
                path.write_bytes(mutated.encode(original.encoding))
                modified += 1
                mutation_counts.update(item.family for item in mutations)
                labels = ", ".join(item.label for item in mutations)
                print(f"{path.name}: {labels}")
        except (
            OSError,
            SyntaxError,
            UnicodeError,
            ValueError,
            tokenize.TokenError,
        ) as error:
            errors += 1
            print(f"ERROR {path.name}: {error}", file=sys.stderr)

    counts = ", ".join(
        f"{count} {family}" for family, count in sorted(mutation_counts.items())
    )
    print(
        f"Modified {modified}/{len(fixtures)} fixtures"
        + (f" ({counts})" if counts else "")
    )
    if errors:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
