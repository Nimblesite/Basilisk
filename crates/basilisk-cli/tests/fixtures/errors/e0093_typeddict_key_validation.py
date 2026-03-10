from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {"name": "Blade Runner", "year": 1982}

movie["director"] = "Ridley Scott"  # E0093: invalid key
movie["year"] = "1982"              # E0093: wrong value type
