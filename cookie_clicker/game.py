"""Cookie game whose authoritative state lives in virtual memory."""

from __future__ import annotations

import copy
from dataclasses import dataclass

from .memory import CODE, GLOBALS, HEAP, HEAP_SIZE, MASK64, Allocation, Heap, Memory, MemoryFault
from .resources import text

TICKS_PER_SECOND = 20
FLAG_PRICE = 1_000_000_000_000
CURSOR_PRICE = 10
MAX_CURSORS = 32

G_COOKIES = 0x00
G_USER = 0x08
G_CURSORS = 0x10
G_CURSOR_COUNT = 0x18
G_MAX_FIELD_SIZE = 0x20
G_FUNCTIONS = 0x28
G_WON = 0x30
G_FLAG_OWNED = 0x38
G_CURSOR_TABLE = 0x100

USER_ID = 0
USER_USERNAME = 8
CURSOR_ID = 0
CURSOR_NAME = 8
CURSOR_DESCRIPTION = 16
CURSOR_ON_CLICKED = 24

FN_ADD_COOKIE = CODE + 0x100
FN_WIN = CODE + 0x180
FUNCTION_TABLE = GLOBALS + 0x300


class GameError(Exception):
    pass


class SegmentationFault(GameError):
    def __init__(self, address: int) -> None:
        self.address = address
        super().__init__(text("error.segfault").replace("{address}", f"0x{address:016X}"))


@dataclass(frozen=True)
class CursorView:
    address: int
    id: int
    name: str
    description: str
    description_ptr: int
    on_clicked: int


@dataclass(frozen=True)
class GameView:
    cookies: int
    username: str
    username_ptr: int
    cookies_ptr: int
    user_ptr: int
    cursor_table_ptr: int
    max_field_size: int
    cursors: tuple[CursorView, ...]
    flag_owned: bool
    won: bool


class Machine:
    def __init__(self, seed: int = 0) -> None:
        self.seed = seed & MASK64
        self.tick = 0
        self.passive_remainder = 0
        self.paused = False
        self.memory = Memory()
        self.memory.map_region(CODE, 0x1000, read=True, write=True, execute=False, kind=".text")
        self.memory.map_region(GLOBALS, 0x1000, read=True, write=True, execute=False, kind=".bss")
        self.memory.map_region(HEAP, HEAP_SIZE, read=True, write=True, execute=False, kind="heap")
        self.memory.write(FN_ADD_COOKIE, b"ADD_COOKIE\x01\0\0\0\0\0", initialize=True)
        self.memory.write(FN_WIN, b"WIN\x01\0\0\0\0", initialize=True)
        self.memory.set_permissions(CODE, read=True, write=False, execute=True)
        self.heap = Heap()
        self._initialize()

    def _initialize(self) -> None:
        username = self.heap.allocate(32, "char username[32]")
        cookies = self.heap.allocate(8, "struct Cookies")
        user = self.heap.allocate(16, "struct User")
        self._write(username, text("default.username").encode()[:31] + b"\0")
        self._write_u64(cookies, 0)
        self._write_u64(user + USER_ID, 1)
        self._write_u64(user + USER_USERNAME, username)
        values = {
            G_COOKIES: cookies,
            G_USER: user,
            G_CURSORS: GLOBALS + G_CURSOR_TABLE,
            G_CURSOR_COUNT: 0,
            G_MAX_FIELD_SIZE: 4096,
            G_FUNCTIONS: FUNCTION_TABLE,
            G_WON: 0,
            G_FLAG_OWNED: 0,
        }
        for offset, value in values.items():
            self._write_u64(GLOBALS + offset, value)
        self._write_u64(FUNCTION_TABLE, FN_ADD_COOKIE)
        self._write_u64(FUNCTION_TABLE + 8, FN_WIN)

    def clone(self) -> "Machine":
        return copy.deepcopy(self)

    def _read_u64(self, address: int) -> int:
        try:
            return self.memory.read_u64(address)
        except MemoryFault as error:
            raise SegmentationFault(error.address) from error

    def _write(self, address: int, data: bytes | bytearray) -> None:
        try:
            self.memory.write(address, data)
        except MemoryFault as error:
            raise SegmentationFault(error.address) from error

    def _write_u64(self, address: int, value: int) -> None:
        self._write(address, int(value & MASK64).to_bytes(8, "little"))

    def read_memory(self, address: int, length: int) -> bytes:
        return self.memory.read(address, length)

    def write_memory(self, address: int, data: bytes | bytearray) -> None:
        self.memory.write(address, data)

    def cookies_address(self) -> int:
        return self._read_u64(GLOBALS + G_COOKIES)

    def user_address(self) -> int:
        return self._read_u64(GLOBALS + G_USER)

    @staticmethod
    def cursor_table_address() -> int:
        return GLOBALS + G_CURSOR_TABLE

    def _read_string(self, address: int, maximum: int) -> str:
        try:
            raw = self.memory.read(address, maximum)
        except MemoryFault as error:
            raise SegmentationFault(error.address) from error
        return raw.split(b"\0", 1)[0].decode("utf-8", errors="replace")

    def cursor_address(self, index: int) -> int:
        count = self._read_u64(GLOBALS + G_CURSOR_COUNT)
        if index < 0 or index >= count or index >= MAX_CURSORS:
            raise GameError(text("error.cursor_missing"))
        return self._read_u64(self.cursor_table_address() + index * 8)

    def view(self) -> GameView:
        cookies_ptr = self.cookies_address()
        user_ptr = self.user_address()
        username_ptr = self._read_u64(user_ptr + USER_USERNAME)
        count = min(self._read_u64(GLOBALS + G_CURSOR_COUNT), MAX_CURSORS)
        cursors: list[CursorView] = []
        for index in range(count):
            address = self._read_u64(self.cursor_table_address() + index * 8)
            description_ptr = self._read_u64(address + CURSOR_DESCRIPTION)
            cursors.append(
                CursorView(
                    address=address,
                    id=self._read_u64(address + CURSOR_ID),
                    name=self._read_string(address + CURSOR_NAME, 8),
                    description=self._read_string(description_ptr, 128),
                    description_ptr=description_ptr,
                    on_clicked=self._read_u64(address + CURSOR_ON_CLICKED),
                )
            )
        return GameView(
            cookies=self._read_u64(cookies_ptr),
            username=self._read_string(username_ptr, 128),
            username_ptr=username_ptr,
            cookies_ptr=cookies_ptr,
            user_ptr=user_ptr,
            cursor_table_ptr=self.cursor_table_address(),
            max_field_size=self._read_u64(GLOBALS + G_MAX_FIELD_SIZE),
            cursors=tuple(cursors),
            flag_owned=bool(self._read_u64(GLOBALS + G_FLAG_OWNED)),
            won=bool(self._read_u64(GLOBALS + G_WON)),
        )

    def click(self) -> None:
        address = self.cookies_address()
        self._write_u64(address, min(MASK64, self._read_u64(address) + 1))

    def buy_flag(self) -> None:
        address = self.cookies_address()
        count = self._read_u64(address)
        if count < FLAG_PRICE:
            raise GameError(text("error.not_enough_cookies"))
        self._write_u64(address, count - FLAG_PRICE)
        self._write_u64(GLOBALS + G_FLAG_OWNED, 1)

    def buy_cursor(self) -> None:
        cookies_address = self.cookies_address()
        cookies = self._read_u64(cookies_address)
        if cookies < CURSOR_PRICE:
            raise GameError(text("error.not_enough_cookies"))
        count = self._read_u64(GLOBALS + G_CURSOR_COUNT)
        if count >= MAX_CURSORS:
            raise GameError(text("error.cursor_table_full"))
        description = self.heap.allocate(64, "char Cursor.description[64]")
        cursor = self.heap.allocate(32, "struct Cursor")
        self._write(description, text("default.cursor.description").encode()[:63] + b"\0")
        self._write_u64(cursor + CURSOR_ID, count + 1)
        name = text("default.cursor.name").encode()[:8].ljust(8, b"\0")
        self._write(cursor + CURSOR_NAME, name)
        self._write_u64(cursor + CURSOR_DESCRIPTION, description)
        self._write_u64(cursor + CURSOR_ON_CLICKED, FN_ADD_COOKIE)
        self._write_u64(self.cursor_table_address() + count * 8, cursor)
        self._write_u64(GLOBALS + G_CURSOR_COUNT, count + 1)
        self._write_u64(cookies_address, cookies - CURSOR_PRICE)

    def rename_user(self, data: bytes) -> None:
        maximum = self._read_u64(GLOBALS + G_MAX_FIELD_SIZE)
        if len(data) + 1 > maximum:
            raise GameError(text("error.field_too_long").replace("{count}", str(maximum)))
        target = self._read_u64(self.user_address() + USER_USERNAME)
        self._write(target, data + b"\0")

    def rename_cursor(self, index: int, data: bytes) -> None:
        self._write(self.cursor_address(index) + CURSOR_NAME, data[:4095] + b"\0")

    def describe_cursor(self, index: int, data: bytes) -> None:
        cursor = self.cursor_address(index)
        target = self._read_u64(cursor + CURSOR_DESCRIPTION)
        self._write(target, data[:4095] + b"\0")

    def invoke_cursor(self, index: int) -> None:
        before = self.clone()
        try:
            cursor = self.cursor_address(index)
            function = self._read_u64(cursor + CURSOR_ON_CLICKED)
            if function == FN_ADD_COOKIE:
                self.click()
            elif function == FN_WIN:
                self._write_u64(GLOBALS + G_WON, 1)
            else:
                raise SegmentationFault(function)
        except Exception:
            self.__dict__.clear()
            self.__dict__.update(before.__dict__)
            raise

    def advance_tick(self) -> None:
        if self.paused:
            return
        self.tick += 1
        self.passive_remainder += 1
        elapsed_seconds, self.passive_remainder = divmod(
            self.passive_remainder, TICKS_PER_SECOND
        )
        for _ in range(elapsed_seconds):
            cursor_count = min(self._read_u64(GLOBALS + G_CURSOR_COUNT), MAX_CURSORS)
            for index in range(cursor_count):
                self.invoke_cursor(index)

    def function_table(self) -> tuple[tuple[str, int], ...]:
        return (
            (text("functions.add_cookie"), self._read_u64(FUNCTION_TABLE)),
            (text("functions.win"), self._read_u64(FUNCTION_TABLE + 8)),
        )

    @property
    def allocations(self) -> tuple[Allocation, ...]:
        return tuple(self.heap.allocations)

    def validate(self) -> None:
        expected = (
            (CODE, 0x1000, True, False, True, ".text"),
            (GLOBALS, 0x1000, True, True, False, ".bss"),
            (HEAP, HEAP_SIZE, True, True, False, "heap"),
        )
        actual = tuple(
            (region.base, region.size, region.read, region.write, region.execute, region.kind)
            for region in self.memory.regions
        )
        if actual != expected:
            raise ValueError("invalid virtual memory map")
        self.heap.validate()
        self.view()

    def export(self) -> dict[str, object]:
        return {
            "seed": self.seed,
            "tick": self.tick,
            "passive_remainder": self.passive_remainder,
            "paused": self.paused,
            "memory": self.memory.export(),
            "heap": self.heap.export(),
        }

    @classmethod
    def restore(cls, state: dict[str, object]) -> "Machine":
        machine = cls.__new__(cls)
        machine.seed = int(state["seed"])
        machine.tick = int(state["tick"])
        machine.passive_remainder = int(state["passive_remainder"])
        machine.paused = bool(state["paused"])
        machine.memory = Memory.restore(state["memory"])  # type: ignore[arg-type]
        machine.heap = Heap.restore(state["heap"])  # type: ignore[arg-type]
        machine.validate()
        return machine
