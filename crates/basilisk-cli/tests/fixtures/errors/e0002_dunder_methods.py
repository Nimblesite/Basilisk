class Vector:
    def __init__(self, x: float, y: float) -> None:
        self.x = x
        self.y = y

    def __repr__(self) -> None:
        return f"Vector({self.x}, {self.y})"

    def __add__(self, other: Vector) -> None:
        return Vector(self.x + other.x, self.y + other.y)

    def __len__(self) -> None:
        return 2
