import sys

from PyQt6.QtCore import QEvent, QObject, QPoint, Qt
from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import (
    QApplication,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

STYLE = """
QWidget#root { background: white; }
QLineEdit, QPushButton#box, QLabel#box {
    background: #efefef;
    border: 1px solid #595959;
    color: black;
    font-size: 16px;
}
QLineEdit { padding: 4px; }
QPushButton#box { padding: 6px; }
QPushButton#box:checked { background: #ffd9a0; }
QLabel#box { padding: 4px; }
QFrame#divider { background: #595959; }
QScrollArea#objects { background: white; border: 1px solid black; }
QWidget#objectsInner { background: white; }
"""

LIST_TEXT_STYLE = "color: #595959; font-size: 14px;"


class PointerTip(QLabel):
    """Dark hover box: <italic type> <name> <red underlined address>."""

    def __init__(self, parent: QWidget):
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.setTextFormat(Qt.TextFormat.RichText)
        self.setFont(QFont("Courier New", 13))
        self.setStyleSheet("background: #595959; padding: 10px 30px;")
        self.hide()

    def set_info(self, ctype: str, name: str, addr: str):
        self.setText(
            f"<i style='color:#d9d9d9'>{ctype}</i> "
            f"<span style='color:#ffffff'>{name}</span> "
            f"<span style='color:#ff0000'><u>{addr}</u></span>"
        )
        self.adjustSize()


class MainWindow(QWidget):
    def __init__(self, pointer_mode: bool = False):
        super().__init__()
        self.setObjectName("root")
        self.setWindowTitle("Cookie Clicker")
        self.setStyleSheet(STYLE)
        self.resize(560, 520)

        # widget -> (type, name, address) shown while hovering in pointer mode
        self._pointer_info: dict[QWidget, tuple[str, str, str]] = {}

        root = QHBoxLayout(self)
        root.setContentsMargins(20, 20, 20, 20)
        root.setSpacing(16)
        root.addLayout(self._build_left())

        divider = QFrame()
        divider.setObjectName("divider")
        divider.setFixedWidth(1)
        root.addWidget(divider)

        root.addLayout(self._build_right())

        # hover tooltip (child of the window, so it can overlap other widgets)
        self.tip = PointerTip(self)

        self._register_pointer("name_edit", self.name_edit, "char*", "name", "0x5500020")
        self._register_pointer("count", self.count_label, "int", "cookies", "0x5500030")
        self._register_pointer("list", self.objects_header, "Cursor*[]", "cursor", "0x400010")
        self._register_pointer("c1", self.cursor1_label, "Cursor*", "cursor", "0x5500980")
        self._register_pointer("c2", self.cursor2_label, "Cursor*", "cursor", "0x5500a00")

        self.pointer_btn.toggled.connect(self._on_pointer_toggled)
        self.pointer_btn.setChecked(pointer_mode)

    # ---------- left column ----------
    def _build_left(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)
        center = Qt.AlignmentFlag.AlignHCenter

        self.name_edit = QLineEdit()
        self.name_edit.setPlaceholderText("your name: (editable)")
        self.name_edit.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.name_edit.setFixedWidth(240)
        col.addWidget(self.name_edit, alignment=center)

        self.cookie_btn = QPushButton("big cookie png\n(clickable)")
        self.cookie_btn.setFixedSize(240, 215)
        self.cookie_btn.setFont(QFont("Courier New", 14))
        self.cookie_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.cookie_btn.setStyleSheet(
            """
            QPushButton {
                background: #ffab40;
                border: 1px solid #595959;
                border-radius: 36px;
                color: #333;
            }
            QPushButton:pressed { background: #f09a2c; }
            """
        )
        col.addWidget(self.cookie_btn, alignment=center)

        self.count_label = QLabel("cookie count: XXX")
        self.count_label.setObjectName("box")
        self.count_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.count_label.setFixedWidth(240)
        col.addWidget(self.count_label, alignment=center)

        row = QHBoxLayout()
        row.setSpacing(10)
        self.debug_btn = QPushButton("[debug\nmode]")
        self.pointer_btn = QPushButton("[pointer view\nmode]")
        for btn in (self.debug_btn, self.pointer_btn):
            btn.setObjectName("box")
            btn.setMinimumHeight(54)
            row.addWidget(btn)
        # only the pointer view button is a toggle in this view
        self.pointer_btn.setCheckable(True)
        col.addLayout(row)

        col.addStretch()
        return col

    # ---------- right column ----------
    def _build_right(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)

        self.buy_cursor_btn = QPushButton("buy cursor: [10 cookies]")
        self.buy_flag_btn = QPushButton("buy flag: 1 trillion cookie")
        for btn in (self.buy_cursor_btn, self.buy_flag_btn):
            btn.setObjectName("box")
            btn.setMinimumHeight(44)
            col.addWidget(btn)

        self.objects = QScrollArea()
        self.objects.setObjectName("objects")
        self.objects.setWidgetResizable(True)
        self.objects.setFixedSize(245, 330)

        inner = QWidget()
        inner.setObjectName("objectsInner")
        lay = QVBoxLayout(inner)
        lay.setContentsMargins(10, 8, 10, 8)
        lay.setSpacing(0)

        self.objects_header = QLabel("objects list:")
        self.cursor1_label = QLabel(
            "<b>cursor (id 1):</b><br><i><b>description</b></i><br>"
        )
        self.cursor2_label = QLabel(
            "<b>cursor (id 2):</b><br><i><b>description</b></i><br>"
        )
        for lbl in (self.objects_header, self.cursor1_label, self.cursor2_label):
            lbl.setStyleSheet(LIST_TEXT_STYLE)
            lbl.setTextFormat(Qt.TextFormat.RichText)
            lay.addWidget(lbl)
        lay.addStretch()

        self.objects.setWidget(inner)
        col.addWidget(self.objects, alignment=Qt.AlignmentFlag.AlignHCenter)

        col.addStretch()
        return col

    # ---------- pointer view mode ----------
    def _register_pointer(self, _key, widget: QWidget, ctype: str, name: str, addr: str):
        self._pointer_info[widget] = (ctype, name, addr)
        widget.installEventFilter(self)

    def _on_pointer_toggled(self, on: bool):
        self.pointer_view = on
        if not on:
            self.tip.hide()

    def eventFilter(self, obj: QObject, event: QEvent) -> bool:
        if obj in self._pointer_info:
            if event.type() == QEvent.Type.Enter and self.pointer_btn.isChecked():
                self.show_tip(obj)
            elif event.type() == QEvent.Type.Leave:
                self.tip.hide()
        return super().eventFilter(obj, event)

    def show_tip(self, widget: QWidget):
        self.tip.set_info(*self._pointer_info[widget])

        # anchor just under the hovered widget, overlapping its bottom edge
        origin = widget.mapTo(self, QPoint(0, 0))
        x = origin.x() + 12
        y = origin.y() + widget.height() - 4

        # keep the box inside the window
        x = max(0, min(x, self.width() - self.tip.width()))
        y = max(0, min(y, self.height() - self.tip.height()))

        self.tip.move(x, y)
        self.tip.raise_()
        self.tip.show()


if __name__ == "__main__":
    app = QApplication(sys.argv)
    # pass --pointer to start with pointer view mode already enabled
    win = MainWindow(pointer_mode="--pointer" in sys.argv)
    win.show()
    sys.exit(app.exec())
