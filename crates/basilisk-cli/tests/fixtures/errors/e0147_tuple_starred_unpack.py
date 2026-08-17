t1: tuple[int, *tuple[str]] = (1, "")
t1 = (1, "", "")  # E: too many elements for *tuple[str]
