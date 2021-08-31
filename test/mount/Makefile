DEPS_FILE := mnt_test
include ../test_common.mk

EXTRA_C_FLAGS :=
EXTRA_LINK_FLAGS :=
BIN_ARGS :=

mnt_test:
	@mkdir -p $(BUILD_DIR)/test/$@/mnt_sefs
	@mkdir -p $(BUILD_DIR)/test/$@/mnt_unionfs/upper
	@mkdir -p $(BUILD_DIR)/test/$@/mnt_unionfs/lower
	@mkdir -p $(BUILD_DIR)/test/$@/mnt_hostfs