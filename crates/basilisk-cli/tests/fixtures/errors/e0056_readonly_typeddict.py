from typing import TypedDict
from typing_extensions import ReadOnly


class Config(TypedDict):
    name: str
    version: ReadOnly[str]


cfg: Config = {"name": "test", "version": "1.0"}
cfg["version"] = "2.0"
