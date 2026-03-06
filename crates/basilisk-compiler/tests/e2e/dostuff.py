def add(a: int, b: int) -> int:
    return a + b


def greet(name: str) -> str:
    return f"hello, {name}"


def fizzbuzz(n: int) -> str:
    if n % 15 == 0:
        return "FizzBuzz"
    elif n % 3 == 0:
        return "Fizz"
    elif n % 5 == 0:
        return "Buzz"
    else:
        return str(n)


print(add(2, 3))
print(greet("basilisk"))
for i in range(1, 16):
    print(fizzbuzz(i))
