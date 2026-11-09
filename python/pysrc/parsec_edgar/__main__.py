"""The ``parsec-edgar`` command: ``parsec-edgar --help`` or ``python -m parsec_edgar --help``."""

import signal
import sys

from ._core import run_cli


def main() -> None:
    # The command runs in Rust without returning to Python until it ends, so
    # Python's own Ctrl-C handler would never run. Let Ctrl-C and a closed
    # output pipe end the process the way they end any command-line tool.
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    if hasattr(signal, "SIGPIPE"):
        signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    sys.exit(run_cli(["parsec-edgar", *sys.argv[1:]]))


if __name__ == "__main__":
    main()
