# BSK-E0034: Final violation — reassignment of Final-annotated names
from typing import Final

MAX_SIZE: Final = 100
MIN_SIZE: Final = 1
TIMEOUT: Final = 30
RETRIES: Final = 3
HOST: Final = "localhost"
PORT: Final = 8080
DEBUG: Final = False
VERSION: Final = "1.0.0"
ENCODING: Final = "utf-8"
DELIMITER: Final = ","

# Each function below reassigns a module-level Final — E0034

def bad01() -> None:
    global MAX_SIZE
    MAX_SIZE = 200  # reassignment of Final

def bad02() -> None:
    global MIN_SIZE
    MIN_SIZE = 0  # reassignment of Final

def bad03() -> None:
    global TIMEOUT
    TIMEOUT = 60  # reassignment of Final

def bad04() -> None:
    global RETRIES
    RETRIES = 5  # reassignment of Final

def bad05() -> None:
    global HOST
    HOST = "example.com"  # reassignment of Final

def bad06() -> None:
    global PORT
    PORT = 443  # reassignment of Final

def bad07() -> None:
    global DEBUG
    DEBUG = True  # reassignment of Final

def bad08() -> None:
    global VERSION
    VERSION = "2.0.0"  # reassignment of Final

def bad09() -> None:
    global ENCODING
    ENCODING = "latin-1"  # reassignment of Final

def bad10() -> None:
    global DELIMITER
    DELIMITER = ";"  # reassignment of Final

# Function-local Final reassignments

def local01() -> None:
    x: Final = 10
    x = 20  # reassignment of local Final

def local02() -> None:
    name: Final = "alice"
    name = "bob"  # reassignment of local Final

def local03() -> None:
    flag: Final = True
    flag = False  # reassignment of local Final

def local04() -> None:
    count: Final = 0
    count = 1  # reassignment of local Final

def local05() -> None:
    limit: Final = 50
    limit = 100  # reassignment of local Final

def local06() -> None:
    ratio: Final = 0.5
    ratio = 0.8  # reassignment of local Final

def local07() -> None:
    label: Final = "start"
    label = "end"  # reassignment of local Final

def local08() -> None:
    size: Final = 128
    size = 256  # reassignment of local Final

def local09() -> None:
    key: Final = "abc"
    key = "xyz"  # reassignment of local Final

def local10() -> None:
    value: Final = 42
    value = 99  # reassignment of local Final
