class Point:
    def __init__(self, x: float, y: float) -> None:
        self.x: float = x
        self.y: float = y

    def distance_to(self, other: "Point") -> float:
        dx: float = self.x - other.x
        dy: float = self.y - other.y
        return (dx * dx + dy * dy) ** 0.5

    def __str__(self) -> str:
        return f"Point({self.x}, {self.y})"


class Rectangle:
    def __init__(self, origin: Point, width: float, height: float) -> None:
        self.origin: Point = origin
        self.width: float = width
        self.height: float = height

    def area(self) -> float:
        return self.width * self.height

    def perimeter(self) -> float:
        return 2.0 * (self.width + self.height)


p1: Point = Point(0.0, 0.0)
p2: Point = Point(3.0, 4.0)
print(p1)
print(p2)
print(p1.distance_to(p2))

rect: Rectangle = Rectangle(Point(1.0, 1.0), 5.0, 3.0)
print(rect.area())
print(rect.perimeter())
