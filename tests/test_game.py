import unittest

from cookie_clicker.game import (
    CURSOR_DESCRIPTION,
    CURSOR_ON_CLICKED,
    FLAG_PRICE,
    FN_ADD_COOKIE,
    FN_WIN,
    FUNCTION_TABLE,
    G_CURSOR_COUNT,
    G_CURSORS,
    MAX_CURSORS,
    USER_ID,
    USER_USERNAME,
    Machine,
    SegmentationFault,
)
from cookie_clicker.memory import CODE, GLOBALS, HEAP, MemoryFault
from cookie_clicker.resources import text


def funded_machine() -> Machine:
    machine = Machine(1)
    for _ in range(10):
        machine.click()
    machine.buy_cursor()
    return machine


class GameTests(unittest.TestCase):
    def test_fresh_layout_and_code_contract(self) -> None:
        machine = Machine(1)
        view = machine.view()
        self.assertEqual(HEAP, 0x0552_00000)
        self.assertEqual(view.cookies, 0)
        self.assertEqual(view.username_ptr, HEAP)
        self.assertEqual(view.cookies_ptr, HEAP + 32)
        self.assertEqual(view.user_ptr, HEAP + 40)
        self.assertEqual(machine.memory.read_u64(view.user_ptr + USER_ID), 1)
        self.assertEqual(machine.memory.read_u64(view.user_ptr + USER_USERNAME), HEAP)
        self.assertEqual(machine.memory.read_u64(GLOBALS + G_CURSORS), GLOBALS + 0x100)
        self.assertEqual(machine.memory.read(FN_ADD_COOKIE, 10), b"ADD_COOKIE")
        with self.assertRaises(MemoryFault):
            machine.write_memory(FN_WIN, b"X")

    def test_cursor_cost_and_one_per_second(self) -> None:
        machine = funded_machine()
        self.assertEqual(machine.view().cookies, 0)
        for _ in range(20):
            machine.advance_tick()
        self.assertEqual(machine.view().cookies, 1)

    def test_passive_production_dispatches_each_cursor_callback(self) -> None:
        machine = funded_machine()
        cursor = machine.view().cursors[0].address
        machine.write_memory(cursor + CURSOR_ON_CLICKED, FN_WIN.to_bytes(8, "little"))
        for _ in range(20):
            machine.advance_tick()
        view = machine.view()
        self.assertTrue(view.won)
        self.assertEqual(view.cookies, 0)

    def test_direct_memory_edit_updates_game(self) -> None:
        machine = Machine(1)
        address = machine.cookies_address()
        machine.write_memory(address, FLAG_PRICE.to_bytes(8, "little"))
        self.assertEqual(machine.view().cookies, FLAG_PRICE)

    def test_username_overflow_reaches_cookies(self) -> None:
        machine = Machine(1)
        machine.rename_user(b"A" * 32 + FLAG_PRICE.to_bytes(8, "little"))
        self.assertEqual(machine.view().cookies, FLAG_PRICE)

    def test_rename_preserves_username_pointer(self) -> None:
        machine = Machine(1)
        pointer = machine.view().username_ptr
        machine.rename_user(b"new user")
        self.assertEqual(machine.view().username, "new user")
        self.assertEqual(machine.view().username_ptr, pointer)

    def test_flag_and_win_are_separate(self) -> None:
        machine = Machine(1)
        machine.write_memory(machine.cookies_address(), FLAG_PRICE.to_bytes(8, "little"))
        machine.buy_flag()
        self.assertTrue(machine.view().flag_owned)
        self.assertFalse(machine.view().won)

    def test_function_table_is_live_name_address_table(self) -> None:
        machine = Machine(1)
        self.assertEqual(
            machine.function_table(),
            ((text("functions.add_cookie"), FN_ADD_COOKIE), (text("functions.win"), FN_WIN)),
        )
        machine.write_memory(FUNCTION_TABLE, FN_WIN.to_bytes(8, "little"))
        self.assertEqual(machine.function_table()[0][1], FN_WIN)

    def test_direct_callback_and_intended_overflow_call_win(self) -> None:
        direct = funded_machine()
        cursor = direct.view().cursors[0].address
        direct.write_memory(cursor + CURSOR_ON_CLICKED, FN_WIN.to_bytes(8, "little"))
        for _ in range(20):
            direct.advance_tick()
        self.assertTrue(direct.view().won)

        exploit = funded_machine()
        cursor = exploit.view().cursors[0].address
        exploit.rename_cursor(0, b"12345678" + (cursor + CURSOR_ON_CLICKED).to_bytes(8, "little"))
        exploit.describe_cursor(0, FN_WIN.to_bytes(8, "little"))
        for _ in range(20):
            exploit.advance_tick()
        self.assertTrue(exploit.view().won)

    def test_invalid_callback_reverts(self) -> None:
        machine = funded_machine()
        cursor = machine.view().cursors[0].address
        machine.write_memory(cursor + CURSOR_ON_CLICKED, ((1 << 64) - 1).to_bytes(8, "little"))
        before = machine.clone()
        with self.assertRaises(SegmentationFault):
            machine.invoke_cursor(0)
        self.assertEqual(machine.export(), before.export())

    def test_invalid_data_pointer_faults(self) -> None:
        machine = funded_machine()
        cursor = machine.view().cursors[0].address
        machine.write_memory(cursor + CURSOR_DESCRIPTION, ((1 << 64) - 1).to_bytes(8, "little"))
        with self.assertRaises(SegmentationFault):
            machine.view()

    def test_pointer_replacement_and_fake_struct(self) -> None:
        machine = funded_machine()
        view = machine.view()
        machine.write_memory(
            view.user_ptr + USER_USERNAME,
            view.cursors[0].description_ptr.to_bytes(8, "little"),
        )
        self.assertEqual(machine.view().username, text("default.cursor.description"))

        machine = Machine(1)
        view = machine.view()
        fake = (
            (7).to_bytes(8, "little")
            + b"FAKE\0\0\0\0"
            + view.cookies_ptr.to_bytes(8, "little")
            + FN_ADD_COOKIE.to_bytes(8, "little")
        )
        machine.write_memory(view.username_ptr, fake)
        machine.write_memory(machine.cursor_table_address(), view.username_ptr.to_bytes(8, "little"))
        machine.write_memory(GLOBALS + G_CURSOR_COUNT, (1).to_bytes(8, "little"))
        machine.describe_cursor(0, FLAG_PRICE.to_bytes(8, "little"))
        self.assertEqual(machine.view().cookies, FLAG_PRICE)

    def test_cursor_table_capacity(self) -> None:
        machine = Machine(1)
        machine.write_memory(machine.cookies_address(), ((1 << 64) - 1).to_bytes(8, "little"))
        for _ in range(MAX_CURSORS):
            machine.buy_cursor()
        self.assertEqual(len(machine.view().cursors), MAX_CURSORS)


if __name__ == "__main__":
    unittest.main()
