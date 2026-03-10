def sum_list(nums: list[int]) -> int:
    total: int = 0
    for n in nums:
        total = total + n
    return total


def filter_even(nums: list[int]) -> list[int]:
    result: list[int] = []
    for n in nums:
        if n % 2 == 0:
            result.append(n)
    return result


def map_double(nums: list[int]) -> list[int]:
    result: list[int] = []
    for n in nums:
        result.append(n * 2)
    return result


data: list[int] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
print(sum_list(data))
print(filter_even(data))
print(map_double([1, 2, 3]))
print(len(data))
print(data[0])
print(data[-1])
