# Cookie Clicker Pwn

A Python/Qt Cookie Clicker binary-exploitation workshop.

## Download an executable

After pushing these changes, open [GitHub Actions](https://github.com/sherlockholmese/cookie-clicker-heap-lab/actions/workflows/ci.yml),
select a successful **CI** run, and download your platform under **Artifacts**:

| Artifact | Platform | Launch after extracting |
| --- | --- | --- |
| `cookie-clicker-pwn-windows-x64` | Windows x64 | `cookie-clicker-pwn.exe` |
| `cookie-clicker-pwn-linux-x64` | Linux x64 (Ubuntu 22.04 or newer) | `./cookie-clicker-pwn` |
| `cookie-clicker-pwn-macos-arm64` | macOS, Apple Silicon | `cookie-clicker-pwn.app` |
| `cookie-clicker-pwn-macos-x64` | macOS, Intel | `cookie-clicker-pwn.app` |

On Windows, extract the downloaded ZIP and open the `.exe`. On macOS and Linux,
extract the ZIP, then the `.tar.gz` inside it (`tar -xzf <filename>.tar.gz`).
The inner archive preserves executable permissions and macOS app symlinks.
Python does not need to be installed. Linux still needs desktop OpenGL/EGL libraries
(on Ubuntu: `sudo apt-get install libegl1 libopengl0 libxcb-cursor0`).
These builds are unsigned; macOS and Windows may require approval to open them.

CI tests and builds on every push and pull request. You can also use **Run workflow**
on the Actions page once the workflow is on the default branch. Downloads are retained
for 30 days and require signing into GitHub.

## Build locally

Run on the operating system and architecture you want to build for:

```sh
python -m pip install . -r requirements-build.txt
python scripts/build_executable.py
```

The executable (and a distribution archive on macOS/Linux) is written to `dist/`.
