from dataclasses import dataclass

@dataclass(match_args=False)
class DC4:
    x: int

DC4.__match_args__  # Should generate BSK-E0059