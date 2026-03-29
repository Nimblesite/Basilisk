from dataclasses import InitVar, dataclass


@dataclass
class DC1:
    x: InitVar[int]
    y: InitVar[str]

    def __post_init__(self, x: int, y: int) -> None:  # E0095: y should be str
        pass


dc1 = DC1(1, "")
dc1.x  # E0095: cannot access InitVar field as attribute
