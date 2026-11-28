"""Build a native executable and archive it with its executable permissions intact."""

from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[1]
NAME = "cookie-clicker-pwn"


def main() -> None:
    is_macos = sys.platform == "darwin"
    is_windows = sys.platform == "win32"
    subprocess.run(
        [
            sys.executable,
            "-m",
            "PyInstaller",
            "--noconfirm",
            "--clean",
            "--noupx",
            "--name", NAME,
            "--paths", str(ROOT),
            "--distpath", str(ROOT / "dist"),
            "--workpath", str(ROOT / "build" / "pyinstaller"),
            "--specpath", str(ROOT / "build"),
            "--collect-data", "cookie_clicker",
            "--onedir" if is_macos else "--onefile",
            *(["--windowed"] if is_macos or is_windows else []),
            str(ROOT / "scripts" / "executable_entry.py"),
        ],
        cwd=ROOT,
        check=True,
    )

    bundle = ROOT / "dist" / (f"{NAME}.app" if is_macos else f"{NAME}.exe" if is_windows else NAME)
    executable = bundle / "Contents" / "MacOS" / NAME if is_macos else bundle
    # Run outside the checkout so imports/resources must come from the bundle.
    # --help imports the app (including Qt and copy resources), then exits.
    with tempfile.TemporaryDirectory() as workdir:
        subprocess.run([str(executable), "--help"], cwd=workdir, check=True, timeout=60)

    # GitHub's artifact ZIP can hold the Windows executable directly.
    if is_windows:
        print(f"Built {bundle}", flush=True)
        return

    system = "macos" if is_macos else "linux"
    architecture = {"amd64": "x64", "x86_64": "x64", "aarch64": "arm64"}.get(
        platform.machine().lower(), platform.machine().lower()
    )
    archive = ROOT / "dist" / f"{NAME}-{system}-{architecture}.tar.gz"
    with tarfile.open(archive, "w:gz") as output:
        output.add(bundle, arcname=bundle.name)
    print(f"Built {archive}", flush=True)


if __name__ == "__main__":
    main()
