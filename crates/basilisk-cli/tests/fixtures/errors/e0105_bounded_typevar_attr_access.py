class C[T: str]:
    def method(self, x: T) -> None:
        x.is_integer()  # E: str does not have is_integer
