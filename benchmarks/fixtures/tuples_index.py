"""Benchmark stress fixture: tuple index out of bounds (`tuples_index_2`).

Fixed-length `tuple[T1, T2, ...]` values indexed with literal integers
outside the valid range [-len, len) are a static error. Each numbered
block repeats one of four rotating variants; every `# error` line is an
out-of-bounds tuple index the checker must flag, and the surrounding
valid indices exercise the bounds check without emitting diagnostics.
"""


def probe_pos_0(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_0(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_0(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid0:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_1(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_1(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_1(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid1:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_2(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_2(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_2(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid2:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_3(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_3(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_3(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid3:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_4(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_4(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_4(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid4:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_5(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_5(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_5(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid5:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_6(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_6(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_6(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid6:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_7(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_7(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_7(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid7:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_8(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_8(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_8(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid8:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_9(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_9(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_9(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid9:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_10(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_10(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_10(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid10:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_11(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_11(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_11(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid11:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_12(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_12(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_12(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid12:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_13(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_13(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_13(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid13:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_14(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_14(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_14(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid14:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_15(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_15(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_15(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid15:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_16(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_16(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_16(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid16:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_17(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_17(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_17(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid17:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_18(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_18(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_18(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid18:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_19(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_19(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_19(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid19:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_20(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_20(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_20(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid20:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_21(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_21(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_21(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid21:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_22(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_22(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_22(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid22:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_23(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_23(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_23(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid23:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_24(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_24(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_24(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid24:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_25(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_25(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_25(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid25:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_26(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_26(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_26(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid26:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_27(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_27(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_27(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid27:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_28(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_28(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_28(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid28:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_29(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_29(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_29(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid29:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_30(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_30(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_30(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid30:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_31(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_31(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_31(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid31:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_32(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_32(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_32(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid32:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_33(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_33(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_33(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid33:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_34(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_34(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_34(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid34:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_35(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_35(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_35(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid35:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_36(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_36(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_36(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid36:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_37(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_37(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_37(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid37:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_38(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_38(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_38(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid38:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_39(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_39(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_39(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid39:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_40(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_40(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_40(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid40:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_41(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_41(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_41(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid41:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_42(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_42(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_42(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid42:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_43(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_43(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_43(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid43:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_44(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_44(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_44(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid44:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_45(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_45(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_45(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid45:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_46(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_46(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_46(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid46:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_47(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_47(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_47(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid47:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_48(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_48(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_48(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid48:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_49(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_49(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_49(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid49:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_50(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_50(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_50(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid50:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_51(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_51(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_51(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid51:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_52(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_52(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_52(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid52:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_53(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_53(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_53(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid53:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_54(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_54(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_54(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid54:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_55(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_55(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_55(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid55:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_56(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_56(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_56(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid56:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_57(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_57(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_57(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid57:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_58(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_58(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_58(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid58:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_59(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_59(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_59(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid59:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_60(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_60(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_60(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid60:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_61(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_61(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_61(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid61:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_62(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_62(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_62(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid62:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_63(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_63(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_63(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid63:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_64(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_64(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_64(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid64:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_65(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_65(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_65(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid65:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_66(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_66(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_66(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid66:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_67(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_67(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_67(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid67:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_68(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_68(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_68(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid68:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_69(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_69(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_69(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid69:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_70(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_70(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_70(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid70:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_71(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_71(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_71(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid71:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_72(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_72(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_72(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid72:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_73(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_73(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_73(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid73:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good


def probe_pos_74(data: tuple[int, str, float]) -> int:
    first: int = data[0]
    bad = data[3]  # error: index 3 out of range for length 3
    worse = data[4]  # error: index 4 out of range for length 3
    return first


def probe_neg_74(pair: tuple[str, bytes]) -> None:
    last = pair[-1]
    lower = pair[-3]  # error: index -3 out of range for length 2
    lowest = pair[-4]  # error: index -4 out of range for length 2


def probe_len_74(row: tuple[int, int, int, int, int]) -> None:
    edge = row[4]
    over = row[5]  # error: index 5 out of range for length 5
    under = row[-6]  # error: index -6 out of range for length 5


class Grid74:
    def cell(self, coords: tuple[int, int]) -> int:
        good: int = coords[1]
        oob = coords[2]  # error: index 2 out of range for length 2
        neg = coords[-3]  # error: index -3 out of range for length 2
        return good

