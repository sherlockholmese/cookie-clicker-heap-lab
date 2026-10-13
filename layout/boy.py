import sys

from PyQt6.QtCore import Qt
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
QLabel#box { padding: 4px; }
QFrame#divider { background: #595959; }
QScrollArea#objects { background: white; border: 1px solid black; }
QWidget#objectsInner { background: white; }
"""


class MainWindow(QWidget):
    def __init__(self):
        super().__init__()
        self.setObjectName("root")
        self.setWindowTitle("Cookie Clicker")
        self.setStyleSheet(STYLE)
        self.resize(560, 520)

        root = QHBoxLayout(self)
        root.setContentsMargins(20, 20, 20, 20)
        root.setSpacing(16)

        root.addLayout(self._build_left())

        divider = QFrame()
        divider.setObjectName("divider")
        divider.setFixedWidth(1)
        root.addWidget(divider)

        root.addLayout(self._build_right())

    def _build_left(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)
        center = Qt.AlignmentFlag.AlignHCenter

        # your name: (editable)
        self.name_edit = QLineEdit()
        self.name_edit.setPlaceholderText("your name: (editable)")
        self.name_edit.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.name_edit.setFixedWidth(240)
        col.addWidget(self.name_edit, alignment=center)

        # big cookie png (clickable)
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

        # cookie count: XXX
        self.count_label = QLabel("cookie count: XXX")
        self.count_label.setObjectName("box")
        self.count_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.count_label.setFixedWidth(240)
        col.addWidget(self.count_label, alignment=center)

        # [debug mode] [pointer view mode]
        row = QHBoxLayout()
        row.setSpacing(10)
        self.debug_btn = QPushButton("[debug\nmode]")
        self.pointer_btn = QPushButton("[pointer view\nmode]")
        for btn in (self.debug_btn, self.pointer_btn):
            btn.setObjectName("box")
            btn.setMinimumHeight(54)
            row.addWidget(btn)
        col.addLayout(row)

        col.addStretch()
        return col

    def _build_right(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)

        # buy buttons
        self.buy_cursor_btn = QPushButton("buy cursor: [10 cookies]")
        self.buy_flag_btn = QPushButton("buy flag: 1 trillion cookie")
        for btn in (self.buy_cursor_btn, self.buy_flag_btn):
            btn.setObjectName("box")
            btn.setMinimumHeight(44)
            col.addWidget(btn)

        # objects list (scrollable box)
        self.objects = QScrollArea()
        self.objects.setObjectName("objects")
        self.objects.setWidgetResizable(True)
        self.objects.setFixedSize(245, 330)

        inner = QWidget()
        inner.setObjectName("objectsInner")
        inner_layout = QVBoxLayout(inner)
        inner_layout.setContentsMargins(5, 5, 5, 5)
        inner_layout.setSpacing(2)

        text = QLabel(
            "objects list:<br>"
            "<b>cursor (id 1):</b><br><i><b>description</b></i><br><br>"
            "<b>cursor (id 2):</b><br><i><b>description</b></i>"
        )
        text.setStyleSheet("color: #595959; font-size: 14px;")
        text.setTextFormat(Qt.TextFormat.RichText)
        text.setWordWrap(True)
        inner_layout.addWidget(text)
        inner_layout.addStretch()

        self.objects.setWidget(inner)
        col.addWidget(self.objects, alignment=Qt.AlignmentFlag.AlignHCenter)

        col.addStretch()
        return col


if __name__ == "__main__":
    app = QApplication(sys.argv)
    win = MainWindow()
    win.show()
    sys.exit(app.exec())
