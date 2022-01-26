include ../../src/sgxenv.mk

SRC_FILES := $(shell find . -type f -name '*.rs') Cargo.toml
RUST_TARGET_DIR := $(BUILD_DIR)/internal/tools/genconf/cargo-target
RUST_OUT_DIR := $(BUILD_DIR)/bin
TARGET_BINARY := $(RUST_OUT_DIR)/gen_internal_conf

CONF_FEATURES :=
ifeq ($(MS_BUFFER), 1)
    CONF_FEATURES += ms_buffer
endif

.PHONY: all clean

all: $(SRC_FILES)
ifeq ($(OCCLUM_RELEASE_BUILD), 1)
	@RUSTC_BOOTSTRAP=1 cargo build --release --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR) --features "$(CONF_FEATURES)"
	@echo "CARGO (release) => gen_internal_conf"
else
	@RUSTC_BOOTSTRAP=1 cargo build --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR) --features "$(CONF_FEATURES)"
	@echo "CARGO (debug) => gen_internal_conf"
endif

clean:
	@cargo clean --target-dir=$(RUST_TARGET_DIR)
	@-$(RM) -f $(TARGET_BINARY)
