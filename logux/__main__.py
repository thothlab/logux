"""Entry point — `python -m logux` or `logux` command."""

from __future__ import annotations

import asyncio
import sys

from .cli.shell import LoguxShell


def main() -> None:
    shell = LoguxShell()
    try:
        asyncio.run(shell.run())
    except KeyboardInterrupt:
        pass
    sys.exit(0)


if __name__ == "__main__":
    main()
