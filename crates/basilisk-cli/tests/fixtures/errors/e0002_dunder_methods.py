class Vector:
    def __init__(self: Vector, x: float, y: float):
        self.x = x
        self.y = y

    def __repr__(self: Vector):
        return f"Vector({self.x}, {self.y})"

    def __add__(self: Vector, other: Vector):
        return Vector(self.x + other.x, self.y + other.y)

    def __len__(self: Vector):
        return 2
