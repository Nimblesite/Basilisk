import sys

if sys.version_info >= (3, 11):
    def new_feature(x: int) -> int:
        return x
else:
    def new_feature(x: int):
        return x
