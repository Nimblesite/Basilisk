class Config:
    host = "localhost"
    port = 8080
    debug = False
    connection: Any = create_connection()
