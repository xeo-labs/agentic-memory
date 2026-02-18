"""Allow running as: python -m validation_7b"""

from validation_7b.run_all import main
import sys

sys.exit(0 if main() else 1)
