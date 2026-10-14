"""Checked 64-bit virtual memory and deterministic teaching heap."""

from __future__ import annotations

from dataclasses import dataclass

CODE = 0x0040_0000
GLOBALS = 0x5510_0000
HEAP = 0x5520_0000
HEAP_SIZE = 64 * 1024
MASK64 = (1 << 64) - 1


class MemoryFault(Exception):
    def __init__(self, address: int, message: str = "unmapped address") -> None:
        self.address = address
        super().__init__(f"{message} 0x{address:016X}")


@dataclass(frozen=True)
class Region:
    base: int
    size: int
    read: bool
    write: bool
    execute: bool
    kind: str

    def contains(self, address: int, length: int = 1) -> bool:
        return length >= 0 and self.base <= address and address + length <= self.base + self.size


@dataclass(frozen=True)
class Allocation:
    id: int
    base: int
    size: int
    alignment: int
    live: bool
    generation: int
    semantic_type: str

    def contains(self, address: int) -> bool:
        return self.base <= address < self.base + self.size


@dataclass(frozen=True)
class FreeBlock:
    base: int
    size: int


class Memory:
    def __init__(self) -> None:
        self.regions: list[Region] = []
        self._storage: dict[int, bytearray] = {}

    def map_region(
        self, base: int, size: int, *, read: bool, write: bool, execute: bool, kind: str
    ) -> None:
        if size <= 0 or any(not (base + size <= r.base or r.base + r.size <= base) for r in self.regions):
            raise ValueError("invalid or overlapping memory region")
        region = Region(base, size, read, write, execute, kind)
        self.regions.append(region)
        self.regions.sort(key=lambda item: item.base)
        self._storage[base] = bytearray(size)

    def set_permissions(self, base: int, *, read: bool, write: bool, execute: bool) -> None:
        for index, region in enumerate(self.regions):
            if region.base == base:
                self.regions[index] = Region(base, region.size, read, write, execute, region.kind)
                return
        raise MemoryFault(base)

    def _locate(self, address: int, length: int, write: bool = False) -> tuple[Region, int]:
        if address < 0 or length < 0 or address > MASK64 or address + length > MASK64 + 1:
            raise MemoryFault(address)
        for region in self.regions:
            if region.contains(address, length):
                if not region.read or (write and not region.write):
                    raise MemoryFault(address, "permission fault at")
                return region, address - region.base
        raise MemoryFault(address)

    def read(self, address: int, length: int) -> bytes:
        region, offset = self._locate(address, length)
        return bytes(self._storage[region.base][offset : offset + length])

    def write(self, address: int, data: bytes | bytearray, *, initialize: bool = False) -> None:
        region, offset = self._locate(address, len(data), write=not initialize)
        self._storage[region.base][offset : offset + len(data)] = data

    def read_u64(self, address: int) -> int:
        return int.from_bytes(self.read(address, 8), "little")

    def write_u64(self, address: int, value: int, *, initialize: bool = False) -> None:
        self.write(address, int(value & MASK64).to_bytes(8, "little"), initialize=initialize)

    def export(self) -> list[dict[str, object]]:
        return [
            {
                "base": region.base,
                "size": region.size,
                "read": region.read,
                "write": region.write,
                "execute": region.execute,
                "kind": region.kind,
                "data": self._storage[region.base].hex(),
            }
            for region in self.regions
        ]

    @classmethod
    def restore(cls, records: list[dict[str, object]]) -> "Memory":
        memory = cls()
        for record in records:
            base, size = int(record["base"]), int(record["size"])
            memory.map_region(
                base,
                size,
                read=bool(record["read"]),
                write=bool(record["write"]),
                execute=bool(record["execute"]),
                kind=str(record["kind"]),
            )
            data = bytes.fromhex(str(record["data"]))
            if len(data) != size:
                raise ValueError("region payload has the wrong size")
            memory._storage[base][:] = data
        return memory


class Heap:
    def __init__(self, base: int = HEAP, size: int = HEAP_SIZE) -> None:
        if base <= 0 or size <= 0 or base + size > MASK64 + 1:
            raise ValueError("invalid heap layout")
        self.base = base
        self.size = size
        self.next_id = 1
        self.allocations: list[Allocation] = []
        self.free_list: list[FreeBlock] = [FreeBlock(base, size)]

    def allocate(self, size: int, semantic_type: str, alignment: int = 0x20) -> int:
        if size <= 0 or alignment <= 0 or alignment & (alignment - 1):
            raise ValueError("invalid allocation size or alignment")
        if len(self.allocations) >= 65536 or self.next_id > MASK64:
            raise MemoryError("virtual heap exhausted")
        for index, block in enumerate(self.free_list):
            address = (block.base + alignment - 1) & ~(alignment - 1)
            end = address + size
            block_end = block.base + block.size
            if end > block_end:
                continue
            generation = 1 + max(
                (allocation.generation for allocation in self.allocations if allocation.base == address),
                default=0,
            )
            replacement: list[FreeBlock] = []
            if address > block.base:
                replacement.append(FreeBlock(block.base, address - block.base))
            if end < block_end:
                replacement.append(FreeBlock(end, block_end - end))
            self.free_list[index : index + 1] = replacement
            self.allocations.append(
                Allocation(self.next_id, address, size, alignment, True, generation, semantic_type)
            )
            self.next_id += 1
            return address
        raise MemoryError("virtual heap exhausted")

    def allocation_for(self, address: int) -> Allocation | None:
        return next(
            (
                allocation
                for allocation in reversed(self.allocations)
                if allocation.live and allocation.contains(address)
            ),
            None,
        )

    def free(self, address: int) -> None:
        match = next(
            (
                (index, allocation)
                for index, allocation in reversed(list(enumerate(self.allocations)))
                if allocation.base == address and allocation.live
            ),
            None,
        )
        if match is None:
            if any(allocation.base == address for allocation in self.allocations):
                raise ValueError(f"double free at 0x{address:016X}")
            raise ValueError(f"invalid free at 0x{address:016X}")
        index, allocation = match
        self.allocations[index] = Allocation(
            allocation.id,
            allocation.base,
            allocation.size,
            allocation.alignment,
            False,
            allocation.generation,
            allocation.semantic_type,
        )
        self.free_list.append(FreeBlock(allocation.base, allocation.size))
        self.free_list.sort(key=lambda block: block.base)
        merged: list[FreeBlock] = []
        for block in self.free_list:
            if merged and merged[-1].base + merged[-1].size == block.base:
                previous = merged[-1]
                merged[-1] = FreeBlock(previous.base, previous.size + block.size)
            else:
                merged.append(block)
        self.free_list = merged

    def describe_write(
        self, address: int, length: int, origin_address: int | None
    ) -> tuple[int | None, list[int], int]:
        end = min(MASK64 + 1, address + max(0, length))
        origin = self.allocation_for(origin_address) if origin_address is not None else None
        affected = [
            allocation.id
            for allocation in self.allocations
            if allocation.live and address < allocation.base + allocation.size and end > allocation.base
        ]
        overflow = max(0, end - (origin.base + origin.size)) if origin else 0
        return (origin.id if origin else None, affected, overflow)

    def validate(self) -> None:
        end = self.base + self.size
        if self.base <= 0 or self.size <= 0 or end > MASK64 + 1:
            raise ValueError("invalid heap bounds")
        ids: set[int] = set()
        spans: list[tuple[int, int]] = []
        for allocation in self.allocations:
            if (
                allocation.id <= 0
                or allocation.id in ids
                or allocation.id >= self.next_id
                or allocation.generation <= 0
                or allocation.size <= 0
                or allocation.alignment <= 0
                or allocation.alignment & (allocation.alignment - 1)
                or allocation.base % allocation.alignment
                or allocation.base < self.base
                or allocation.base + allocation.size > end
            ):
                raise ValueError("invalid allocation metadata")
            ids.add(allocation.id)
            if allocation.live:
                spans.append((allocation.base, allocation.base + allocation.size))
        previous = self.base
        for block in self.free_list:
            if block.size <= 0 or block.base < previous or block.base + block.size > end:
                raise ValueError("invalid free list")
            previous = block.base + block.size
            spans.append((block.base, previous))
        spans.sort()
        cursor = self.base
        for start, span_end in spans:
            if start != cursor:
                raise ValueError("overlapping or missing heap spans")
            cursor = span_end
        if cursor != end:
            raise ValueError("incomplete heap coverage")

    def export(self) -> dict[str, object]:
        return {
            "base": self.base,
            "size": self.size,
            "next_id": self.next_id,
            "allocations": [allocation.__dict__ for allocation in self.allocations],
            "free_list": [block.__dict__ for block in self.free_list],
        }

    @classmethod
    def restore(cls, record: dict[str, object]) -> "Heap":
        heap = cls(int(record["base"]), int(record["size"]))
        heap.next_id = int(record["next_id"])
        heap.allocations = [Allocation(**item) for item in record["allocations"]]  # type: ignore[arg-type]
        heap.free_list = [FreeBlock(**item) for item in record["free_list"]]  # type: ignore[arg-type]
        heap.validate()
        return heap
