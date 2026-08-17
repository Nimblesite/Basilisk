from __future__ import annotations

import os
import sys
import re
import json
import pathlib
from typing import Optional
from collections import defaultdict
from dataclasses import dataclass


def get_path() -> str:
    return os.getcwd()


def get_version() -> str:
    return sys.version


def find_pattern(text: str, pattern: str) -> Optional[str]:
    match = re.search(pattern, text)
    if match:
        return match.group(0)
    return None
