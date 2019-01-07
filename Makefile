.PHONY: all src test clean

all: src

submodule:
	git submodule init
	git submodule update
	cd deps/sgx_protect_file && make

src:
	@$(MAKE) --no-print-directory -C src

test:
	@$(MAKE) --no-print-directory -C test test

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
