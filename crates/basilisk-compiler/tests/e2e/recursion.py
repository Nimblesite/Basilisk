def factorial(n: int) -> int:
    if n <= 1:
        return 1
    return n * factorial(n - 1)


def fibonacci(n: int) -> int:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    a: int = 0
    b: int = 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b


def power(base: int, exp: int) -> int:
    if exp == 0:
        return 1
    if exp % 2 == 0:
        half: int = power(base, exp // 2)
        return half * half
    return base * power(base, exp - 1)


print(factorial(10))
for i in range(10):
    print(fibonacci(i))
print(power(2, 10))
print(power(3, 5))
