class Service:
    def connect(self, host, port):
        return object()

    def disconnect(self) -> None:
        pass

    def send(self, payload: str):
        return object()
