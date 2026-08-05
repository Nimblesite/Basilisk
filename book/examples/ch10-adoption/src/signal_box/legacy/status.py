"""Legacy status conversion with one visible type-safety debt."""


def status_label(code: int) -> str:
    """Return a display label for a vendor status code."""
    return str(code)


FALLBACK_LABEL = status_label("offline")
