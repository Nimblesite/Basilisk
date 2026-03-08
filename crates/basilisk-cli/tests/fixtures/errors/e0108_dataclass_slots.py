from dataclasses import dataclass


@dataclass(slots=True)
class DC:
    x: int

    def set_y(self) -> None:
        self.y = 3  # E: "y" is not in __slots__


@dataclass
class DC2:
    a: int


DC2.__slots__  # E: __slots__ not defined
