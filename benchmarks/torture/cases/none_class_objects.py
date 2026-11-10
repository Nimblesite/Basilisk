"""`None` the value versus `type(None)` the class object.

The typing spec's special-types chapter
(https://typing.python.org/en/latest/spec/special-types.html#none) treats the
annotation `None` as `type(None)`, but the VALUE `None` is never a class
object and `type(None)` is never the value `None`. The two marked calls are
errors; everything else is legal.
"""


def takes_none(x: None) -> None:
    pass


def takes_type(x: type) -> None:
    pass


takes_none(None)  # OK
takes_none(type(None))  # E
takes_type(type(None))  # OK
takes_type(None)  # E
