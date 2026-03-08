from typing import TypedDict

class Base(TypedDict):
    x: int

class Child(Base):
    x: str
