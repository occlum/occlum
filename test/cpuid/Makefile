DEPS_FILE := test_cpuid.txt
include ../test_common.mk

EXTRA_C_FLAGS :=
EXTRA_LINK_FLAGS :=
BIN_ARGS :=

test_cpuid.txt:
	@/usr/bin/cpuid -r -1 > $@
	@mv $@ $(IMAGE_DIR)/$@
