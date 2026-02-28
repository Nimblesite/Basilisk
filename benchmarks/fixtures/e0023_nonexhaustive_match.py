# BSK-E0023: Non-exhaustive match statement
# Match statements over Literal types without a wildcard arm.

from typing import Literal

Status = Literal["ok", "error", "pending"]
Direction = Literal["north", "south", "east", "west"]
Color = Literal["red", "green", "blue"]
Size = Literal["small", "medium", "large"]
State = Literal["active", "inactive", "suspended"]


def handle_status(s: Status) -> str:
    match s:
        case "ok":
            return "all good"
        case "error":
            return "something failed"
    return "unknown"  # "pending" not handled


def navigate(d: Direction) -> str:
    match d:
        case "north":
            return "going north"
        case "south":
            return "going south"
    return "other"  # "east" and "west" not handled


def pick_color(c: Color) -> int:
    match c:
        case "red":
            return 0xFF0000
        case "green":
            return 0x00FF00
    return 0  # "blue" not handled


def size_to_px(s: Size) -> int:
    match s:
        case "small":
            return 12
        case "medium":
            return 16
    return 0  # "large" not handled


def describe_state(s: State) -> str:
    match s:
        case "active":
            return "running"
    return "other"  # "inactive" and "suspended" not handled


def handle_status_2(s: Status) -> str:
    match s:
        case "ok":
            return "all good"
        case "error":
            return "something failed"
    return "unknown"


def navigate_2(d: Direction) -> str:
    match d:
        case "north":
            return "going north"
        case "south":
            return "going south"
    return "other"


def pick_color_2(c: Color) -> int:
    match c:
        case "red":
            return 0xFF0000
        case "green":
            return 0x00FF00
    return 0


def size_to_px_2(s: Size) -> int:
    match s:
        case "small":
            return 12
        case "medium":
            return 16
    return 0


def describe_state_2(s: State) -> str:
    match s:
        case "active":
            return "running"
    return "other"
