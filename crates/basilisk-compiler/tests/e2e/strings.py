def is_palindrome(s: str) -> bool:
    return s == s[::-1]


def caesar_cipher(text: str, shift: int) -> str:
    result: list[str] = []
    for ch in text:
        if ch.isalpha():
            base: int = ord("a") if ch.islower() else ord("A")
            shifted: int = (ord(ch) - base + shift) % 26 + base
            result.append(chr(shifted))
        else:
            result.append(ch)
    return "".join(result)


def count_vowels(s: str) -> int:
    count: int = 0
    for ch in s.lower():
        if ch in "aeiou":
            count = count + 1
    return count


print(is_palindrome("racecar"))
print(is_palindrome("hello"))
print(caesar_cipher("Hello, World!", 3))
print(caesar_cipher("Khoor, Zruog!", -3))
print(count_vowels("Basilisk"))
print("hello world".upper())
print("  spaces  ".strip())
print("-".join(["a", "b", "c"]))
