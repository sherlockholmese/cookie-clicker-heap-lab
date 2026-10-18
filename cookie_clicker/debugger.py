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
    F32 = ("f32", 4, "float")
    F64 = ("f64", 8, "float")
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
        if self is ValueType.F32:
            return str(struct.unpack("<f", value)[0])
        if self is ValueType.F64:
            return str(struct.unpack("<d", value)[0])
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


class ScanFilter(enum.Enum):
    EXACT = "Exact value"
    EQUAL = "Equal to value"
    NOT_EQUAL = "Not equal to value"
    CHANGED = "Changed"
    UNCHANGED = "Unchanged"
    INCREASED = "Increased"
    DECREASED = "Decreased"
    INCREASED_BY = "Increased by"
    DECREASED_BY = "Decreased by"

    @property
    def needs_value(self) -> bool:
        return self in {
            ScanFilter.EXACT,
            ScanFilter.EQUAL,
            ScanFilter.NOT_EQUAL,
            ScanFilter.INCREASED_BY,
            ScanFilter.DECREASED_BY,
        }


@dataclass
class ScanResult:
    address: int
    previous: bytes


class Scanner:
    def __init__(self) -> None:
        self.results: list[ScanResult] = []
        self.width = 0
        self.initialized = False
        self.byte_pattern = False

    def first(self, backend: Backend, width: int, exact: int | None, aligned: bool) -> None:
        if width not in (1, 2, 4, 8):
            raise ValueError("scan width must be 1, 2, 4, or 8 bytes")
        if exact is not None and not 0 <= exact < 1 << (width * 8):
            raise ValueError("value does not fit the selected width")
        pattern = exact.to_bytes(width, "little") if exact is not None else None
        self._scan(backend, width, pattern, aligned, False)

    def first_bytes(self, backend: Backend, pattern: bytes) -> None:
        if not 1 <= len(pattern) <= 256:
            raise ValueError("text or byte patterns must contain 1 to 256 bytes")
        self._scan(backend, len(pattern), pattern, False, True)

    def _scan(
        self, backend: Backend, width: int, pattern: bytes | None, aligned: bool, byte_pattern: bool
    ) -> None:
        results: list[ScanResult] = []
        for region in backend.memory.regions:
            if not (region.read and region.write):
                continue
            data = backend.read_memory(region.base, region.size)
            step = width if aligned else 1
            for offset in range(0, len(data) - width + 1, step):
                address = region.base + offset
                if aligned and address % width:
                    continue
                value = data[offset : offset + width]
                if pattern is None or value == pattern:
                    results.append(ScanResult(address, value))
        self.results = results
        self.width = width
        self.initialized = True
        self.byte_pattern = byte_pattern

    def filter(self, backend: Backend, rule: ScanFilter, target: int | None = None) -> None:
        if self.byte_pattern:
            raise ValueError("this scan contains text/bytes; use a byte filter")
        if target is not None and not 0 <= target < 1 << (self.width * 8):
            raise ValueError("value does not fit the scan width")
        value = target.to_bytes(self.width, "little") if target is not None else None
        self._filter(backend, rule, value)

    def filter_bytes(
        self, backend: Backend, rule: ScanFilter, target: bytes | None = None
    ) -> None:
        if not self.byte_pattern:
            raise ValueError("this is a number scan")
        if rule not in {
            ScanFilter.EXACT,
            ScanFilter.EQUAL,
            ScanFilter.NOT_EQUAL,
            ScanFilter.CHANGED,
            ScanFilter.UNCHANGED,
        }:
            raise ValueError("text/byte scans do not support ordered filters")
        if target is not None and len(target) != self.width:
            raise ValueError("keep the same byte length when narrowing results")
        self._filter(backend, rule, target)

    def _filter(self, backend: Backend, rule: ScanFilter, target: bytes | None) -> None:
        if not self.initialized:
            raise ValueError("run a first scan before filtering")
        if rule.needs_value and target is None:
            raise ValueError("enter a filter value")
        amount = unsigned(target or b"")
        retained: list[ScanResult] = []
        for result in self.results:
            try:
                now = backend.read_memory(result.address, self.width)
            except MemoryFault:
                continue
            old_value, new_value = unsigned(result.previous), unsigned(now)
            keep = {
                ScanFilter.EXACT: now == target,
                ScanFilter.EQUAL: now == target,
                ScanFilter.NOT_EQUAL: now != target,
                ScanFilter.CHANGED: now != result.previous,
                ScanFilter.UNCHANGED: now == result.previous,
                ScanFilter.INCREASED: new_value > old_value,
                ScanFilter.DECREASED: new_value < old_value,
                ScanFilter.INCREASED_BY: new_value >= old_value and new_value - old_value == amount,
                ScanFilter.DECREASED_BY: old_value >= new_value and old_value - new_value == amount,
            }[rule]
            result.previous = now
            if keep:
                retained.append(result)
        self.results = retained


@dataclass(frozen=True)
class Watch:
    address: int
    kind: ValueType
    name: str

    def value(self, backend: Backend) -> str:
        return self.kind.interpret(backend.read_memory(self.address, self.kind.width))


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
