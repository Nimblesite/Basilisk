from enum import Enum
from typing import Literal


class Pet4(Enum):
    CAT = 1
    converter = lambda x: str(x)  # Non-member (lambda)

    def speak(self) -> None: ...  # Non-member (method)


converter_var: Literal[Pet4.converter]  # E0067 — converter is not an enum member
speak_var: Literal[Pet4.speak]  # E0067 — speak is not an enum member
