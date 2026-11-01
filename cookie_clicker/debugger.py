from __future__ import annotations

import enum
import struct
from dataclasses import dataclass
from typing import Protocol

from .memory import Allocation, Memory, MemoryFault


class Backend(Protocol):
    memory: Memory

    def read_memory(self, address: int, length: int) -> bytes: ...

    @property
    def allocations(self) -> tuple[Allocation, ...]: ...


def parse_u64(value: str) -> int:
    candidate = value.strip().replace("_", "")
    base = 16 if candidate.lower().startswith("0x") else 2 if candidate.lower().startswith("0b") else 10
    try:
        result = int(candidate, base)
    except ValueError as error:
        raise ValueError("enter decimal, 0x hexadecimal, or 0b binary digits") from error
    if not 0 <= result < 1 << 64:
        raise ValueError("value does not fit u64")
    return result


def parse_hex(value: str) -> bytes:
    cleaned = "".join(value.split())
    if len(cleaned) % 2:
        raise ValueError("enter pairs of hexadecimal digits, such as 20 31 01 00")
    try:
        return bytes.fromhex(cleaned)
    except ValueError as error:
        raise ValueError("invalid hexadecimal byte") from error


def unsigned(value: bytes) -> int:
    return int.from_bytes(value[:8].ljust(8, b"\0"), "little")


class ValueType(enum.Enum):
    U8 = ("u8", 1, "unsigned")
    U16 = ("u16", 2, "unsigned")
    U32 = ("u32", 4, "unsigned")
    U64 = ("u64", 8, "unsigned")
    I8 = ("i8", 1, "signed")
    I16 = ("i16", 2, "signed")
    I32 = ("i32", 4, "signed")
    I64 = ("i64", 8, "signed")
    POINTER = ("pointer (u64)", 8, "pointer")
    ASCII = ("ASCII", 16, "ascii")
    UTF8 = ("UTF-8", 16, "utf8")

    def __init__(self, label: str, width: int, category: str) -> None:
        self.label = label
        self.width = width
        self.category = category

    def interpret(self, data: bytes) -> str:
        value = data[: self.width]
        if self.category == "unsigned":
            return str(int.from_bytes(value, "little"))
        if self.category == "signed":
            return str(int.from_bytes(value, "little", signed=True))
        if self.category == "pointer":
            return f"0x{int.from_bytes(value, 'little'):016X}"
        if self.category == "ascii":
            return "".join(chr(byte) if 32 <= byte < 127 else "." for byte in value)
        return value.rstrip(b"\0").decode("utf-8")

    def encode(self, value: str) -> bytes:
        candidate = value.strip()
        invalid = f"enter a valid {self.label} value"
        try:
            if self.category in {"unsigned", "pointer"}:
                number = parse_u64(candidate)
                if number >= 1 << (self.width * 8):
                    raise ValueError(invalid)
                return number.to_bytes(self.width, "little")
            if self.category == "signed":
                number = int(candidate, 10)
                return number.to_bytes(self.width, "little", signed=True)
            if self is ValueType.F32:
                return struct.pack("<f", float(candidate))
            if self is ValueType.F64:
                return struct.pack("<d", float(candidate))
            if self is ValueType.ASCII and not value.isascii():
                raise ValueError("ASCII accepts only ASCII characters")
            return value.encode("ascii" if self is ValueType.ASCII else "utf-8")
        except (OverflowError, ValueError) as error:
            if isinstance(error, ValueError) and str(error).startswith("ASCII"):
                raise
            raise ValueError(invalid) from error


def pointer_label(address: int, allocations: tuple[Allocation, ...]) -> str:
    for allocation in reversed(allocations):
        if allocation.live and allocation.contains(address):
            return f"allocation #{allocation.id} + 0x{address - allocation.base:X}"
    return "no live allocation"


@dataclass(frozen=True)
class Diff:
    address: int
    before: int
    after: int


def diff(snapshot: Memory, backend: Backend) -> list[Diff]:
    changes: list[Diff] = []
    for region in snapshot.regions:
        try:
            before = snapshot.read(region.base, region.size)
            after = backend.read_memory(region.base, region.size)
        except MemoryFault:
            continue
        changes.extend(
            Diff(region.base + offset, old, new)
            for offset, (old, new) in enumerate(zip(before, after, strict=True))
            if old != new
        )
    return changes
