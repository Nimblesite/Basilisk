"""Tests for examples/data_pipeline.py — ETL data pipeline."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from data_pipeline import (
    BaseWriter,
    Column,
    ParquetWriter,
    PartitionKey,
    coerce_field,
    detect_encoding,
    empty_schema,
    read_source,
)


class TestCoerceField:
    def test_str_to_int(self) -> None:
        assert coerce_field("42", int) == 42

    def test_int_to_str(self) -> None:
        assert coerce_field(42, str) == "42"

    def test_str_to_float(self) -> None:
        assert coerce_field("3.14", float) == 3.14


class TestDetectEncoding:
    def test_utf8_bom(self) -> None:
        assert detect_encoding(b"\xef\xbb\xbfhello") == "utf-8-sig"

    def test_utf16_le_bom(self) -> None:
        assert detect_encoding(b"\xff\xfehello") == "utf-16"

    def test_utf16_be_bom(self) -> None:
        assert detect_encoding(b"\xfe\xffhello") == "utf-16"


class TestReadSource:
    def test_returns_empty_list(self) -> None:
        result = read_source("nonexistent.csv")
        assert result == []


class TestColumnHierarchy:
    def test_column_fields(self) -> None:
        col = Column()
        col.name = "id"
        col.dtype = "int"
        col.nullable = False
        assert col.name == "id"

    def test_partition_key_inherits(self) -> None:
        assert issubclass(PartitionKey, Column)

    def test_partition_key_nullable_default(self) -> None:
        pk = PartitionKey()
        assert pk.nullable == 0


class TestWriterHierarchy:
    def test_base_writer_flush(self) -> None:
        writer = BaseWriter()
        assert writer.flush([b"a", b"b"]) == 2

    def test_parquet_writer_flush(self) -> None:
        writer = ParquetWriter()
        assert writer.flush([b"a", b"b"]) == 4

    def test_parquet_is_base(self) -> None:
        assert issubclass(ParquetWriter, BaseWriter)
