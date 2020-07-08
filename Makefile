.PHONY: all submodule githooks src test tools install format format-check clean

all: src

githooks:
	@find .git/hooks -type l -exec rm {} \; && find .githooks -type f -exec ln -sf ../../{} .git/hooks/ \;
	@echo "Add Git hooks that check Rust code format issues before commits and pushes"

OCCLUM_GIT_OPTIONS ?= 
GIT_MIN_VERSION := 2.11.0
GIT_CURRENT_VERSION := $(shell git --version | sed 's/[^0-9.]*//g')
GIT_NEED_PROGRESS := $(shell /bin/echo -e "$(GIT_MIN_VERSION)\n$(GIT_CURRENT_VERSION)" \
			| sort -V | head -n1 | grep -q $(GIT_MIN_VERSION) && echo "true" || echo "false")
# If git version >= min_version, append the `--progress` option to show progress status
ifeq ($(GIT_NEED_PROGRESS), true)
	OCCLUM_GIT_OPTIONS += --progress
else
	OCCLUM_GIT_OPTIONS +=
endif

# Occlum major version
MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' ./src/pal/include/occlum_version.h |  awk '{print $$3}')

# Exclude files when install
EXCLUDE_FILES = "libocclum-libos.so.$(MAJOR_VER_NUM)\$$|libocclum-pal.so.$(MAJOR_VER_NUM)\$$|libocclum-pal.so\$$|.a\$$|occlum-protect-integrity.so.*"

submodule: githooks
	git submodule init
	git submodule update $(OCCLUM_GIT_OPTIONS)
	@# Try to apply the patches. If failed, check if the patches are already applied
	cd deps/rust-sgx-sdk && git apply ../rust-sgx-sdk.patch >/dev/null 2>&1 || git apply ../rust-sgx-sdk.patch -R --check
	cd deps/serde-json-sgx && git apply ../serde-json-sgx.patch >/dev/null 2>&1 || git apply ../serde-json-sgx.patch -R --check

	@# Enclaves used by tools are running in simulation mode by default to run faster.
	@rm -rf build build_sim
	@$(MAKE) SGX_MODE=SIM --no-print-directory -C tools
	@$(MAKE) --no-print-directory -C deps/sefs/sefs-fuse clean
	@$(MAKE) SGX_MODE=SIM --no-print-directory -C deps/sefs/sefs-fuse
	@cp deps/sefs/sefs-fuse/bin/sefs-fuse build/bin
	@cp deps/sefs/sefs-fuse/lib/libsefs-fuse.signed.so build/lib

src:
	@$(MAKE) --no-print-directory -C src

test:
	@$(MAKE) --no-print-directory -C test test

OCCLUM_PREFIX ?= /opt/occlum
install:
	@# Install both libraries for HW mode and SIM mode
	@$(MAKE) SGX_MODE=HW --no-print-directory -C src
	@$(MAKE) SGX_MODE=SIM --no-print-directory -C src

	@echo "Install libraries ..."
	@mkdir -p $(OCCLUM_PREFIX)/build/bin/
	@cp build/bin/* $(OCCLUM_PREFIX)/build/bin
	@mkdir -p $(OCCLUM_PREFIX)/build/lib/
	@# Don't copy libos library and pal library symbolic files to install dir
	@cd build/lib && cp --no-dereference `ls | grep -Ev $(EXCLUDE_FILES)` $(OCCLUM_PREFIX)/build/lib/ && cd -

	@echo "Install headers and miscs ..."
	@mkdir -p $(OCCLUM_PREFIX)/include/
	@cp -r src/pal/include/*.h $(OCCLUM_PREFIX)/include
	@chmod 444 $(OCCLUM_PREFIX)/include/*.h
	@mkdir -p $(OCCLUM_PREFIX)/etc/template/
	@cp etc/template/* $(OCCLUM_PREFIX)/etc/template
	@chmod 444 $(OCCLUM_PREFIX)/etc/template/*
	@echo "Installation is done."

format:
	@$(MAKE) --no-print-directory -C test format
	@$(MAKE) --no-print-directory -C tools format
	@$(MAKE) --no-print-directory -C src format

format-check:
	@$(MAKE) --no-print-directory -C test format-check
	@$(MAKE) --no-print-directory -C tools format-check
	@$(MAKE) --no-print-directory -C src format-check

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
	@$(MAKE) SGX_MODE=SIM --no-print-directory -C src clean
	@$(MAKE) SGX_MODE=SIM --no-print-directory -C test clean
