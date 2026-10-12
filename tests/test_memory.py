import unittest

from cookie_clicker.memory import HEAP, Memory, MemoryFault


class MemoryTests(unittest.TestCase):
    def test_little_endian_pointer_and_permissions(self) -> None:
        memory = Memory()
        memory.map_region(HEAP, 32, read=True, write=True, execute=False, kind="heap")
        memory.write_u64(HEAP, HEAP + 24)
        self.assertEqual(memory.read(HEAP, 8), (HEAP + 24).to_bytes(8, "little"))
        self.assertEqual(memory.read_u64(HEAP), HEAP + 24)
        memory.set_permissions(HEAP, read=True, write=False, execute=True)
        with self.assertRaises(MemoryFault):
            memory.write(HEAP, b"x")

    def test_faulting_write_is_atomic(self) -> None:
        memory = Memory()
        memory.map_region(HEAP, 16, read=True, write=True, execute=False, kind="heap")
        before = memory.read(HEAP, 16)
        with self.assertRaises(MemoryFault):
            memory.write(HEAP + 12, b"too long")
        self.assertEqual(memory.read(HEAP, 16), before)


if __name__ == "__main__":
    unittest.main()
