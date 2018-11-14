.PHONY: all build_src build_test test clean

all: build_src build_test

submodule:
	git submodule init
	git submodule update
	cd deps/sgx_protect_file && make

build_src:
	@$(MAKE) --no-print-directory -C src

build_test:
	@$(MAKE) --no-print-directory -C test

test: build_test
	@$(MAKE) --no-print-directory -C test run

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
