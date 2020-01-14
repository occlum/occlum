.PHONY: all submodule githooks src test tools install clean

all: src

githooks:
	@find .git/hooks -type l -exec rm {} \; && find .githooks -type f -exec ln -sf ../../{} .git/hooks/ \;
	@echo "Add Git hooks that check Rust code format issues before commits and pushes"

GIT_MIN_VERSION := 2.11.0
GIT_CURRENT_VERSION := $(shell git --version | sed 's/[^0-9.]*//g')
GIT_NEED_PROGRESS := $(shell /bin/echo -e "$(GIT_MIN_VERSION)\n$(GIT_CURRENT_VERSION)" \
			| sort -V | head -n1 | grep -q $(GIT_MIN_VERSION) && echo "true" || echo "false")
# If git version >= min_version, append the `--progress` option to show progress status
ifeq ($(GIT_NEED_PROGRESS), true)
	GIT_OPTION := --progress
else
	GIT_OPTION :=
endif

submodule: githooks
	git submodule init
	git submodule update $(GIT_OPTION)
	@# Try to apply the patches. If failed, check if the patches are already applied
	cd deps/rust-sgx-sdk && git apply ../rust-sgx-sdk.patch >/dev/null 2>&1 || git apply ../rust-sgx-sdk.patch -R --check
	cd deps/serde-json-sgx && git apply ../serde-json-sgx.patch >/dev/null 2>&1 || git apply ../serde-json-sgx.patch -R --check
	cd deps/sefs/sefs-fuse && make
	cd tools/ && make

src:
	@$(MAKE) --no-print-directory -C src

test:
	@$(MAKE) --no-print-directory -C test test

OCCLUM_PREFIX ?= /opt/occlum
install:
	install -d $(OCCLUM_PREFIX)/deps/sefs/sefs-fuse/bin/
	install -t $(OCCLUM_PREFIX)/deps/sefs/sefs-fuse/bin/ deps/sefs/sefs-fuse/bin/*
	install -d $(OCCLUM_PREFIX)/build/bin/
	install -t $(OCCLUM_PREFIX)/build/bin/ -D build/bin/*
	install -d $(OCCLUM_PREFIX)/build/lib/
	install -t $(OCCLUM_PREFIX)/build/lib/ -D build/lib/*
	install -d $(OCCLUM_PREFIX)/src/
	install -t $(OCCLUM_PREFIX)/src/ -m 444 src/sgxenv.mk
	install -d $(OCCLUM_PREFIX)/src/libos/
	install -t $(OCCLUM_PREFIX)/src/libos/ -m 444 src/libos/Makefile src/libos/Enclave.lds
	install -d $(OCCLUM_PREFIX)/src/libos/src/builtin/
	install -t $(OCCLUM_PREFIX)/src/libos/src/builtin/ -m 444 src/libos/src/builtin/*
	install -d $(OCCLUM_PREFIX)/include/
	install -t $(OCCLUM_PREFIX)/include/ -m 444 src/pal/include/*
	install -d $(OCCLUM_PREFIX)/etc/template/
	install -t $(OCCLUM_PREFIX)/etc/template/ -m 444 etc/template/*

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
