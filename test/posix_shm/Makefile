include ../test_common.mk

EXTRA_C_FLAGS := -Wno-stringop-truncation -Wno-nonnull
EXTRA_LINK_FLAGS := -lrt
BIN_ARGS :=

CUSTOM_PRE_BUILD := 1

custom_pre_build:
	@cp /lib/x86_64-linux-gnu/librt.so.1 $(BUILD_DIR)/test/image/opt/occlum/glibc/lib/
