from __future__ import annotations


def classify(status: int) -> str:
    match status:
        case 200:
            return "ok"
        case 404:
            return "not found"
        case _:
            return "unknown"


def describe(value: str) -> str:
    match value:
        case "a" | "e" | "i" | "o" | "u":
            return "vowel"
        case _:
            return "consonant"
