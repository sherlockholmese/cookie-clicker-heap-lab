import tempfile
import unittest
from pathlib import Path

from cookie_clicker.game import Machine
from cookie_clicker.app import parser
from cookie_clicker.resources import COPY, cookie_sprite, text
from cookie_clicker.save_state import SaveError, decode, encode, load, save
from cookie_clicker.qt_ui import parse_address, parse_field


class ResourceAndSaveTests(unittest.TestCase):
    def test_raw_byte_notation_and_addresses(self) -> None:
        self.assertEqual(parse_field(r"A\x00\xFF"), bytes((65, 0, 255)))
        with self.assertRaises(ValueError):
            parse_field(r"\xGG")
        self.assertEqual(parse_address("0x5520_0000"), 0x55200000)
        self.assertEqual(parse_address("1234"), 1234)
        self.assertEqual(parser().parse_args(["--seed", "0"]).seed, 0)

    def test_every_copy_key_is_present_and_sprite_is_complete(self) -> None:
        self.assertNotIn("", COPY)
        self.assertTrue(all(text(key) != f"<{key}>" for key in COPY))
        width, height, palette, rows = cookie_sprite()
        self.assertEqual((width, height), (16, 16))
        self.assertEqual(len(rows), 16)
        self.assertTrue(any(transparent for _color, transparent in palette.values()))

    def test_save_round_trip_and_damage_rejection(self) -> None:
        machine = Machine(7)
        machine.click()
        encoded = encode(machine)
        self.assertEqual(decode(encoded).export(), machine.export())
        damaged = bytearray(encoded)
        damaged[-1] ^= 1
        with self.assertRaises(SaveError):
            decode(bytes(damaged))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "workshop.ccpy"
            save(path, machine)
            self.assertEqual(load(path).export(), machine.export())


if __name__ == "__main__":
    unittest.main()
