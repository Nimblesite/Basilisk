class Vector:
    def __init__(self, x: float, y: float):
        self.x = x
        self.y = y

    def __repr__(self):
        return f"Vector({self.x}, {self.y})"

    def __add__(self, other: Vector):
        return Vector(self.x + other.x, self.y + other.y)

    def __len__(self):
        return 2
