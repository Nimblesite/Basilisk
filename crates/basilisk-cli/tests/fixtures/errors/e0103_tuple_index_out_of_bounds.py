v: tuple[int, str, list[bool]] = (3, "hi", [True])
v[4]   # E: index 4 out of range for 3-element tuple
v[-4]  # E: index -4 out of range for 3-element tuple
