class Base:
    def process(self, data: str) -> str:
        return data


class Child(Base):
    def process(self, data: Any) -> None:
        return data.upper()

    def extra(self, value: Any) -> None:
        return value
