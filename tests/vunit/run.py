from pathlib import Path
from vunit import VUnit

ROOT = Path(__file__).parent

vu = VUnit.from_argv()
vu.add_vhdl_builtins()

test_lib = vu.add_library("test_lib")

test_lib.add_source_files(ROOT / "vhlib" / "util" / "*.vhd")
test_lib.add_source_files(ROOT / "vhlib" / "stream" / "*.vhd")

test_lib.add_source_files(ROOT / "*.vhd")


vu.main()
