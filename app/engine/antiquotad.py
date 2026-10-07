"""Entry point of the bundled engine: `antiquotad serve` (and every other antiquota command)."""
import sys

from antiquota.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
