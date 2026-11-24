"""Exercise the Qt application with QT_QPA_PLATFORM=offscreen."""

import unittest

from PyQt6.QtTest import QTest

from cookie_clicker.game import CURSOR_PRICE, Machine
from cookie_clicker.qt_ui import CookieApp
from cookie_clicker.resources import text


class GuiSmokeTest(unittest.TestCase):
    def test_game_and_debugger(self) -> None:
        machine = Machine()
        application = CookieApp(machine)
        window = application.window
        try:
            window.show()
            QTest.qWait(150)
            self.assertTrue(window.isVisible())
            self.assertGreater(machine.tick, 0, "The Qt event loop must run the game timer")
            # Keep balances deterministic while exercising purchases.
            window._timer.stop()

            self.assertFalse(window.buy_cursor_btn.isEnabled())
            for _ in range(CURSOR_PRICE):
                window.cookie_btn.click()
            self.assertEqual(machine.view().cookies, CURSOR_PRICE)
            self.assertEqual(window.count_label.text(), f"{CURSOR_PRICE} {text('game.cookies')}")
            self.assertTrue(window.buy_cursor_btn.isEnabled())

            window.buy_cursor_btn.click()
            self.assertEqual(machine.view().cookies, 0)
            self.assertEqual(len(machine.view().cursors), 1)
            self.assertEqual(len(window.cursor_rows), 1)

            window.pointer_btn.click()
            self.assertTrue(window.pointer_view)
            window.debug_btn.click()
            application.app.processEvents()
            debugger = window.memory_window
            self.assertIsNotNone(debugger)
            self.assertTrue(debugger.isVisible())
            self.assertTrue(debugger.cell_values, "The memory view must render data")

            debugger.mode_btn.click()
            self.assertFalse(debugger.qwords)
            window.cookie_btn.click()
            application.app.processEvents()
            self.assertEqual(machine.view().cookies, 1)
            self.assertEqual(window.count_label.text(), f"1 {text('game.cookies')}")
        finally:
            window._timer.stop()
            application.app.closeAllWindows()
            application.app.processEvents()


if __name__ == "__main__":
    unittest.main(verbosity=2)
