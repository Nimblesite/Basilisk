from typing import Self, Generic, TypeVar

T = TypeVar("T")


class Shape:
    def method2(self) -> Self:
        return Shape()  # E0078 — should return self, not Shape()

    @classmethod
    def cls_method2(cls) -> Self:
        return Shape()  # E0078 — should return cls(), not Shape()


class Container(Generic[T]):
    def foo(self, other: Self[int]) -> None:  # E0078 — Self is not subscriptable
        pass
