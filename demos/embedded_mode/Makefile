SGX_SDK ?= /opt/intel/sgxsdk
SGX_MODE ?= HW

ifneq ($(SGX_MODE), HW)
	OCCLUM_LIB := /opt/occlum/build_sim/lib
else
	OCCLUM_LIB := /opt/occlum/build/lib
endif

.PHONY: all build_src test clean

all: occlum_context

occlum_context: build_src
	@mkdir -p occlum_context
	@cd occlum_context && \
		occlum init && \
		cp ../trusted_memcpy_bench/build/trusted_memcpy_bench image/bin/ && \
		occlum build
	@cp -r occlum_context/.occlum .occlum

build_src:
	@$(MAKE) --no-print-directory -C trusted_memcpy_bench
	@$(MAKE) --no-print-directory -C bench_driver

TOTAL_BYTES := 10000000000 # 10GB
test:
	LD_LIBRARY_PATH=$(OCCLUM_LIB):$(SGX_SDK)/sdk_libs RUST_BACKTRACE=1 \
		./bench_driver/build/bench_driver $(TOTAL_BYTES)

clean:
	@$(MAKE) --no-print-directory -C trusted_memcpy_bench clean
	@$(MAKE) --no-print-directory -C bench_driver clean
	@rm -rf .occlum occlum_context
