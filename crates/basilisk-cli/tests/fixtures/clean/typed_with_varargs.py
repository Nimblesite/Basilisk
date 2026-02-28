def log(*messages: str, level: str) -> None:
    pass


def merge(**kwargs: int) -> int:
    return sum(kwargs.values())


def mixed(first: str, *rest: int, key: str) -> str:
    return first
