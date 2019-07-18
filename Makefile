.PHONY: all src test clean

all: src

submodule:
	git submodule init
	git submodule update
	cd deps/sefs/sefs-fuse && make

src:
	@$(MAKE) --no-print-directory -C src

test:
	@$(MAKE) --no-print-directory -C test test

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
