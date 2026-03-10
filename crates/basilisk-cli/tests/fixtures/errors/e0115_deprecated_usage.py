from typing_extensions import deprecated


@deprecated("Use new_func instead")
def old_func() -> None:
    pass


old_func()  # E: use of deprecated function
