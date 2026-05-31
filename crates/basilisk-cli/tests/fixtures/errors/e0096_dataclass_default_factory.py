from dataclasses import dataclass, field

@dataclass
class DC:
    a: Any = field(default_factory=str)  # E0096: str() -> str, not int
