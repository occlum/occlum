DEPS_FILE := test_cpuid.txt
include ../test_common.mk

EXTRA_C_FLAGS :=
EXTRA_LINK_FLAGS :=
BIN_ARGS :=

test_cpuid.txt:
	@/usr/bin/cpuid -r -1 | sed -r '/^\s*0x[^ 08][0-9a-f]{7}\s/d' > $@
	@mv $@ $(IMAGE_DIR)/$@
