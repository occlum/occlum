include ../sgxenv.mk

DEBUG=1

LIBOS_ENCLAVE := libocclum.signed.so
LIBOS_SO := libocclum.so # Link $(LIBOS_A), $(C_OBJS) and all dependencies
LIBOS_A := liblibos.a # Built from Rust code

RUST_SRCS := $(wildcard src/*.rs)
C_SRCS := $(sort $(filter-out src/Enclave_t.c, $(wildcard src/*.c))) src/Enclave_t.c
C_OBJS := $(C_SRCS:.c=.o)
S_SRCS := $(wildcard src/*.S)
S_OBJS := $(S_SRCS:.S=.o)
EDL_C_SRCS := src/Enclave_t.c src/Enclave_t.h

ENCLAVE_CONFIG := Enclave_config.xml
ENCLAVE_KEY := Enclave_private.pem

C_FLAGS := $(SGX_CFLAGS_T) -fno-stack-protector
_Other_Link_Flags := -L../../deps/rust-sgx-sdk/compiler-rt/ -L.
_Other_Enclave_Libs := -lcompiler-rt-patch -llibos -lsgx_tprotected_fs
LINK_FLAGS := $(SGX_LFLAGS_T)

.PHONY: all compiler-rt clean

all: $(LIBOS_ENCLAVE)

$(LIBOS_ENCLAVE): $(LIBOS_SO)
	@$(SGX_ENCLAVE_SIGNER) sign -key $(ENCLAVE_KEY) -enclave $^ -out $@ -config $(ENCLAVE_CONFIG)
	@echo "SIGN => $@"

$(LIBOS_SO): compiler-rt $(LIBOS_A) $(C_OBJS) $(S_OBJS)
	@$(CC) $(C_OBJS) $(S_OBJS) -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"

$(C_OBJS): %.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(S_OBJS): %.o: %.S
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(EDL_C_SRCS): $(SGX_EDGER8R) ../Enclave.edl
	@cd src/ && $(SGX_EDGER8R) --trusted ../../Enclave.edl --search-path $(SGX_SDK)/include --search-path ../../../deps/rust-sgx-sdk/edl/
	@echo "GEN  => $(EDL_C_SRCS)"

compiler-rt:
	@$(MAKE) --no-print-directory -C ../../deps/rust-sgx-sdk/compiler-rt/ 2> /dev/null

ifeq ($(DEBUG), 1)
$(LIBOS_A): $(RUST_SRCS)
	RUSTC_BOOTSTRAP=1 cargo build
	cp ./target/debug/$(LIBOS_A) $(LIBOS_A)
else
$(LIBOS_A): $(RUST_SRCS)
	RUSTC_BOOTSTRAP=1 cargo build --release
	cp ./target/release/$(LIBOS_A) $(LIBOS_A)
endif

clean:
	@cargo clean
	@-$(RM) $(LIBOS_ENCLAVE) $(LIBOS_SO) $(LIBOS_A) $(C_OBJS) $(S_OBJS) $(EDL_C_SRCS)
