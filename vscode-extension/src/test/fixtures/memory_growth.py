"""Memory profiling e2e fixture: grows a module-level cache in steps."""
import time

CACHE = []


def allocate_chunk():
    for _i in range(300):
        CACHE.append("x" * 5000)
    return len(CACHE)


def main():
    first = allocate_chunk()
    second = allocate_chunk()
    third = allocate_chunk()
    time.sleep(2)
    print("DONE", first, second, third)


if __name__ == "__main__":
    main()
