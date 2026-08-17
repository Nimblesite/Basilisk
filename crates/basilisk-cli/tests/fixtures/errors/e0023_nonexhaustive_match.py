def classify(status: int) -> str:
    match status:
        case 200:
            return "ok"
        case 404:
            return "not found"
    return "unknown"
