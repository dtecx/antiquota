"""Entry point of the bundled engine: `agyloopd serve` (and every other agyloop command)."""
import sys

from agyloop.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
