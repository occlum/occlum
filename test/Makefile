.PHONY: all build_rusgx_stub build_src clean test

all: build_rusgx_stub build_src

build_src: build_rusgx_stub
	@$(MAKE) -C src

build_rusgx_stub:
	@$(MAKE) -C rusgx_stub

test: all
	@$(MAKE) -C src test

test-without-rusgx: all
	@$(MAKE) -C src test-without-rusgx

clean:
	@$(MAKE) -C rusgx_stub clean
	@$(MAKE) -C src clean
