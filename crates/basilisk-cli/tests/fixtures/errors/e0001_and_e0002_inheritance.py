class Base:
    def process(self: Base, data: str) -> str:
        return data


class Child(Base):
    def process(self, data):
        return data.upper()

    def extra(self, value):
        return value
