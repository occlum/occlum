include ../../src/sgxenv.mk

SRC_FILES := $(shell find . -type f -name '*.rs') Cargo.toml
RUST_TARGET_DIR := $(BUILD_DIR)/internal/tools/init_aecs/cargo-target
RUST_OUT_DIR := $(BUILD_DIR)/bin
TARGET_BINARY := $(RUST_OUT_DIR)/init_aecs

.PHONY: all clean

all: $(SRC_FILES)
	@RUSTC_BOOTSTRAP=1 cargo build --release --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR)
	@echo "CARGO (release) => init_aecs"

clean:
	@cargo clean --target-dir=$(RUST_TARGET_DIR)
	@-$(RM) -f $(TARGET_BINARY)
