"""Versioned, checksummed JSON snapshots using only the Python standard library."""

from __future__ import annotations

import json
import os
import struct
import tempfile
import zlib
from pathlib import Path

from .game import GameError, Machine
from .memory import MemoryFault

MAGIC = b"CCPYPY\r\n"
VERSION = 1
MAX_FILE_SIZE = 2 * 1024 * 1024
HEADER = struct.Struct("<8sIQI")


class SaveError(Exception):
    pass


def encode(machine: Machine) -> bytes:
    machine.validate()
    payload = json.dumps(machine.export(), separators=(",", ":"), sort_keys=True).encode("utf-8")
    if len(payload) > MAX_FILE_SIZE - HEADER.size:
        raise SaveError("save file exceeds size limit")
    return HEADER.pack(MAGIC, VERSION, len(payload), zlib.crc32(payload)) + payload


def decode(data: bytes) -> Machine:
    if len(data) < HEADER.size or len(data) > MAX_FILE_SIZE:
        raise SaveError("truncated or oversized save file")
    magic, version, length, checksum = HEADER.unpack_from(data)
    payload = data[HEADER.size :]
    if magic != MAGIC:
        raise SaveError("invalid save magic")
    if version != VERSION:
        raise SaveError(f"unsupported save version {version}")
    if length != len(payload):
        raise SaveError("save payload length mismatch")
    if zlib.crc32(payload) != checksum:
        raise SaveError("save checksum mismatch")
    try:
        state = json.loads(payload.decode("utf-8"))
        return Machine.restore(state)
    except (KeyError, TypeError, ValueError, UnicodeError, GameError, MemoryFault) as error:
        raise SaveError(f"invalid save payload: {error}") from error


def save(path: str | Path, machine: Machine) -> None:
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{destination.name}.", dir=destination.parent)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(encode(machine))
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
    except Exception:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def load(path: str | Path) -> Machine:
    destination = Path(path)
    if destination.stat().st_size > MAX_FILE_SIZE:
        raise SaveError("save file exceeds size limit")
    return decode(destination.read_bytes())
