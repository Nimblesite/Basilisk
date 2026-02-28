def read_file(path: str) -> str:
    with open(path) as f:
        def read_all(handle: object) -> str:
            return handle.read()  # type: ignore[union-attr]
        return read_all(f)


def write_file(path: str, content: str) -> None:
    with open(path, "w") as f:
        f.write(content)
