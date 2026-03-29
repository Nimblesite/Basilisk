from typing import TypedDict


class Movie(TypedDict):
    name: str
    year: int


movie: Movie = {"name": "Blade Runner", "year": 1982}

if isinstance(movie, Movie):  # E0088 — TypedDict cannot be used in isinstance
    pass
