"""Offscreen PyQt6 smoke test for the ported main menu.

Run headless with:  QT_QPA_PLATFORM=offscreen PYTHONPATH=. python3 tests/gui_smoke.py
"""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PyQt6.QtWidgets import QApplication

from cookie_clicker.game import CURSOR_PRICE, Machine
from cookie_clicker.qt_ui import MainWindow, parse_field


def main() -> None:
    app = QApplication.instance() or QApplication([])
    win = MainWindow(Machine(1))
    win.show()
    app.processEvents()

    # The passive timer must not auto-run during the test.
    win._timer.stop()

    # Click the cookie and confirm the model + label update.
    win.execute(win.machine.click)
    assert win.machine.view().cookies == 1
    assert "1" in win.count_label.text()

    # Fund and buy a cursor; the objects list should gain a row.
    win.machine.write_memory(win.machine.cookies_address(), (CURSOR_PRICE).to_bytes(8, "little"))
    win.execute(win.machine.buy_cursor)
    assert len(win.machine.view().cursors) == 1
    assert len(win.cursor_rows) == 1

    # Rename via the game surface (parse_field handles \xNN escapes too).
    win.name_edit.setText("hacker")
    win._change_username()
    assert win.machine.view().username == "hacker"
    assert win.name_edit.text() == "hacker"
    assert parse_field(r"A\x00") == b"A\x00"

    # Pointer view toggle shows/hides the hover tip without error.
    win.pointer_btn.setChecked(True)
    win.show_tip(win.count_label)
    app.processEvents()
    assert win.tip.isVisible()
    win.pointer_btn.setChecked(False)
    app.processEvents()
    assert not win.tip.isVisible()

    # Memory window: open, verify dump + navigation + write + flash + follow.
    from cookie_clicker.memory import HEAP

    win._open_debugger()
    app.processEvents()
    mem = win.memory_window
    assert mem is not None and mem.isVisible()
    assert mem.base == HEAP
    assert mem.addr_cells[0].text()  # first address rendered

    # section shortcut jumps
    from cookie_clicker.memory import GLOBALS

    mem.jump_to(GLOBALS)
    assert mem.base == GLOBALS
    assert mem.section_buttons["memory.section.bss"].isChecked()

    # a .bss pointer field is detected and followable
    assert mem.pointer_field_base(GLOBALS) is not None
    mem._click(GLOBALS)  # click a pointer -> navigates to its target (heap)
    assert mem.base != GLOBALS

    # qwords is the default view (2 columns); toggling switches to 16 bytes.
    assert mem.qwords is True
    assert len(mem.value_cells[0]) == 2
    mem.mode_btn.setChecked(False)
    mem._toggle_mode()
    assert mem.qwords is False
    assert len(mem.value_cells[0]) == 16

    # write a byte through the debugger updates the shared machine + flashes
    mem.jump_to(win.machine.cookies_address())
    cookies = win.machine.cookies_address()
    mem.selected = cookies
    mem.edit_value.setText("7F")
    mem._write_selected()
    assert win.machine.view().cookies == 0x7F
    # a subsequent model change flashes the changed cell
    win.machine.write_memory(cookies, (0x80).to_bytes(8, "little"))
    mem.refresh()
    assert cookies in mem.flashing

    # back to the default qwords view
    mem.mode_btn.setChecked(True)
    mem._toggle_mode()
    assert len(mem.value_cells[0]) == 2

    # Function table shows only in .text view and lists all five functions.
    from cookie_clicker.memory import CODE, HEAP

    mem.jump_to(CODE)
    assert mem.fn_table.isVisible()
    assert not mem.heap_scroll.isVisible()
    assert mem.fn_table.text().count("0x") == 5

    # Heap view shows the scrollable struct list instead of the function table.
    mem.jump_to(HEAP)
    assert mem.heap_scroll.isVisible()
    assert not mem.fn_table.isVisible()
    # at least the Cookies, username-less structs: Cookies, User, Cursor
    struct_entries = mem.heap_list_layout.count() - 1  # minus the trailing stretch
    assert struct_entries >= 2

    # Win state swaps the surface to the win label.
    from cookie_clicker.game import G_WON, GLOBALS as G

    win.machine.write_memory(G + G_WON, (1).to_bytes(8, "little"))
    win.refresh()
    assert win.win_label.isVisible()
    assert not win.cookie_btn.isVisible()

    mem.close()
    win.close()
    print("qt gui_smoke PASSED")


if __name__ == "__main__":
    main()
