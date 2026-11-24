import unittest

from cookie_clicker.game import CURSOR_PRICE, TICKS_PER_SECOND, GameError, Machine


class GameTest(unittest.TestCase):
    def test_cursor_purchase_and_passive_income(self) -> None:
        machine = Machine()
        for _ in range(CURSOR_PRICE):
            machine.click()
        machine.buy_cursor()
        self.assertEqual(machine.view().cookies, 0)
        self.assertEqual(len(machine.view().cursors), 1)

        for _ in range(TICKS_PER_SECOND * 5):
            machine.advance_tick()
        self.assertEqual(machine.view().cookies, 1)

    def test_unaffordable_cursor_leaves_game_unchanged(self) -> None:
        machine = Machine()
        before = machine.export()
        with self.assertRaises(GameError):
            machine.buy_cursor()
        self.assertEqual(machine.export(), before)


if __name__ == "__main__":
    unittest.main()
