def bad() -> int:
    yield 1  # E: generator with non-generator return type
