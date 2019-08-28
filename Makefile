.PHONY: all submodule githooks src test clean

all: src

githooks:
	@find .git/hooks -type l -exec rm {} \; && find .githooks -type f -exec ln -sf ../../{} .git/hooks/ \;
	@echo "Add Git hooks that check Rust code format issues before commits and pushes"

submodule: githooks
	git submodule init
	git submodule update
	cd deps/rust-sgx-sdk && git apply ../rust-sgx-sdk.patch
	cd deps/sefs/sefs-fuse && make
	cd tools/protect-integrity && make

src:
	@$(MAKE) --no-print-directory -C src

test:
	@$(MAKE) --no-print-directory -C test test

clean:
	@$(MAKE) --no-print-directory -C src clean
	@$(MAKE) --no-print-directory -C test clean
