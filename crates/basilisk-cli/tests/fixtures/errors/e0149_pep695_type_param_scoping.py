class ClassB[S: Sequence[T], T]:  # E: S's bound references T (later param)
    pass


class ClassE[T]:
    def method1[T](self) -> None:  # E: method re-defines class type param T
        pass
