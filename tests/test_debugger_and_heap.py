import copy
import unittest

from cookie_clicker.debugger import (
    ScanFilter,
    Scanner,
    ValueType,
    diff,
    parse_hex,
    parse_u64,
    pointer_label,
)
from cookie_clicker.game import Machine
from cookie_clicker.memory import HEAP, Heap


class DebuggerAndHeapTests(unittest.TestCase):
    def test_heap_reuse_coalescing_and_write_description(self) -> None:
        heap = Heap(HEAP, 1024)
        first = heap.allocate(32, "Name", 8)
        second = heap.allocate(64, "Stats", 16)
        self.assertEqual(second, first + 32)
        self.assertEqual(heap.allocation_for(second + 63).base, second)
        origin, affected, overflow = heap.describe_write(first, 40, first)
        self.assertEqual(origin, 1)
        self.assertEqual(affected, [1, 2])
        self.assertEqual(overflow, 8)
        heap.free(first)
        reused = heap.allocate(16, "Effect", 8)
        self.assertEqual(reused, first)
        self.assertEqual(heap.allocation_for(reused).generation, 2)
        heap.free(reused)
        with self.assertRaisesRegex(ValueError, "double free"):
            heap.free(reused)
        heap.free(second)
        heap.validate()
        self.assertEqual(len(heap.free_list), 1)
        self.assertEqual(heap.free_list[0].size, 1024)

    def test_typed_values_parsers_and_pointer_labels(self) -> None:
        self.assertEqual(parse_u64("0b111_1111"), 127)
        self.assertEqual(parse_hex("20 31 01 00"), bytes((32, 49, 1, 0)))
        self.assertEqual(ValueType.U64.encode("0x55200000"), (0x55200000).to_bytes(8, "little"))
        self.assertEqual(ValueType.I16.interpret(b"\xff\xff"), "-1")
        with self.assertRaises(ValueError):
            ValueType.U8.encode("256")
        machine = Machine(1)
        label = pointer_label(machine.view().username_ptr, machine.allocations)
        self.assertIn("allocation #1", label)

    def test_scans_narrow_and_diff_memory(self) -> None:
        machine = Machine(1)
        address = machine.cookies_address()
        machine.write_memory(address, (127).to_bytes(8, "little"))
        scanner = Scanner()
        scanner.first(machine, 8, 127, True)
        self.assertTrue(any(result.address == address for result in scanner.results))
        machine.write_memory(address, (128).to_bytes(8, "little"))
        scanner.filter(machine, ScanFilter.EXACT, 128)
        self.assertEqual([result.address for result in scanner.results], [address])

        machine.write_memory(machine.view().username_ptr, "饼干".encode())
        scanner.first_bytes(machine, "饼干".encode())
        self.assertTrue(any(result.address == machine.view().username_ptr for result in scanner.results))

        snapshot = copy.deepcopy(machine.memory)
        machine.write_memory(address, b"*")
        self.assertEqual(diff(snapshot, machine)[0].address, address)


if __name__ == "__main__":
    unittest.main()
