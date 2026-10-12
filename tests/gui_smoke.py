"""Display-backed smoke test for the native tkinter surfaces."""

from cookie_clicker.game import Machine
from cookie_clicker.resources import text
from cookie_clicker.ui import CookieApp


def main() -> None:
    app = CookieApp(Machine(1))
    app.root.update_idletasks()
    assert app.cookie_image is not None
    app.memory.open()
    app.root.update_idletasks()
    assert len(app.memory.cells) == 32
    assert len(app.memory.cells[0]) == 16
    assert app.memory.mode_value is not None
    app.memory.mode_value.set("qwords")
    app.memory._change_mode()
    assert len(app.memory.cells[0]) == 2
    app.memory.jump_to(0x55100300)
    app.root.update_idletasks()
    function_widgets = tuple(app.memory.function_frame.winfo_children())
    first_memory_cell = app.memory.cells[0][0]
    app.memory.refresh()
    app.memory.refresh()
    assert tuple(app.memory.function_frame.winfo_children()) == function_widgets
    assert app.memory.cells[0][0] is first_memory_cell
    app.machine.write_memory(app.machine.cookies_address(), (10).to_bytes(8, "little"))
    app.machine.buy_cursor()
    view = app.machine.view()
    assert app.memory.field_color(view.username_ptr) == app.memory.field_color(
        view.username_ptr + 31
    )
    cursor_address = view.cursors[0].address
    assert app.memory.field_color(cursor_address + 8) == app.memory.field_color(
        cursor_address + 15
    )
    assert app.memory.field_color(cursor_address) != app.memory.field_color(
        cursor_address + 8
    )
    app.pointer_mode.set(True)
    app.refresh()
    cursor_buttons = [
        child
        for row in app.cursor_list.winfo_children()
        for child in row.winfo_children()
        if child.winfo_class() == "TButton"
    ]
    assert len(cursor_buttons) == 2
    assert text("game.cursor.edit") in {button.cget("text") for button in cursor_buttons}
    first_user_widget = app.user_label
    app.refresh()
    assert app.user_label is first_user_widget
    app.pointer_tooltip.show_pointer(
        app.user_label,
        "username",
        app.machine.view().username_ptr,
        app.memory.open,
    )
    pending = app.pointer_tooltip.owner.after(1, lambda: None)
    app.pointer_tooltip.pending = pending
    app.pointer_tooltip.show_pointer(
        app.user_label,
        "username",
        app.machine.view().username_ptr,
        app.memory.open,
    )
    app.root.update()
    assert app.pointer_tooltip.window is not None
    app.pointer_tooltip.schedule_hide()
    popup_frame = app.pointer_tooltip.window.winfo_children()[0]
    popup_frame.winfo_children()[0].event_generate("<Enter>")
    app.root.after(650, app.root.quit)
    app.root.mainloop()
    assert app.pointer_tooltip.window is not None
    app.edit_cursor(0)
    app.root.update_idletasks()
    assert app.cursor_editors
    assert app.memory.base == app.machine.view().cursors[0].address & ~0xF
    app.root.destroy()


if __name__ == "__main__":
    main()
