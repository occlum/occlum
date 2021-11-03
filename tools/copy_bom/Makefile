include ../../src/sgxenv.mk

SRC_FILES := $(shell find . -type f -name '*.rs') Cargo.toml
RUST_TARGET_DIR := $(BUILD_DIR)/internal/tools/copy_bom/cargo-target
RUST_OUT_DIR := $(BUILD_DIR)/bin
TARGET_BINARY := $(RUST_OUT_DIR)/copy_bom

.PHONY: all clean

all: $(SRC_FILES)
ifeq ($(OCCLUM_RELEASE_BUILD), 1)
	@RUSTC_BOOTSTRAP=1 cargo build --release --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR)
	@echo "CARGO (release) => copy_bom"
else
	@RUSTC_BOOTSTRAP=1 cargo build --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR)
	@echo "CARGO (debug) => copy_bom"
endif

clean:
	@cargo clean --target-dir=$(RUST_TARGET_DIR)
	@-$(RM) -f $(TARGET_BINARY)
