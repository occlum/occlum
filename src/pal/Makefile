include ../sgxenv.mk

EDL_Gen_Files := Enclave_u.c Enclave_u.h

Srcs := $(sort $(filter-out Enclave_u.c, $(wildcard *.c)))
Objs := $(Srcs:.c=.o) Enclave_u.o
Bin := pal

C_Flags := $(SGX_CFLAGS_U)
Link_Flags := $(SGX_LFLAGS_U)
Link_Flags += -L$(RUST_SGX_SDK_DIR)/sgx_ustdc/ -lsgx_ustdc -lsgx_uprotected_fs


.PHONY: all
all: $(Bin)

$(Bin): $(EDL_Gen_Files) $(Objs) sgx_ustdc
	@$(CC) $(Objs) -o $@ $(Link_Flags)
	@echo "LINK =>  $@"

$(EDL_Gen_Files): $(SGX_EDGER8R) ../Enclave.edl
	@$(SGX_EDGER8R) --untrusted ../Enclave.edl --search-path $(SGX_SDK)/include --search-path $(RUST_SGX_SDK_DIR)/edl/
	@echo "GEN  => $(EDL_Gen_Files)"

$(Objs): %.o: %.c
	@$(CC) $(C_Flags) -c $< -o $@
	@echo "CC <= $@"

#
# Compile dependencies in Rust SGX SDK
#
.PHONY: sgx_ustdc
sgx_ustdc:
	@$(MAKE) --no-print-directory -C $(RUST_SGX_SDK_DIR)/sgx_ustdc/ > /dev/null

.PHONY: clean
clean:
	@-$(RM) -f $(EDL_Gen_Files) $(Objs) $(Bin)
