include ../buildenv.mk

EDL_Gen_Files := Enclave_t.c Enclave_t.h

Enclave_Config := Enclave_config.xml
Enclave_Key := Enclave_private.pem

Test_Srcs := $(sort $(filter-out Enclave_t.c, $(wildcard *.c)))
Test_Targets := $(Test_Srcs:%.c=test-%)
Test_Objs := $(Test_Srcs:.c=.o) Enclave_t.o
Test_Enclaves := $(Test_Srcs:.c=.so)
Test_Signed_Enclaves := $(Test_Srcs:.c=.signed.so)

C_Flags := $(SGX_CFLAGS_T)


Other_Link_Flags := -L../deps/rust-sgx-sdk/compiler-rt/ -L../src/libos/
Other_Enclave_Libs := -lcompiler-rt-patch -lrusgx
Link_Flags := $(SGX_LFLAGS_T)



all: build




.PHONY: build
build: compiler-rt $(Test_Signed_Enclaves)


$(Test_Signed_Enclaves): %.signed.so: %.so
	@$(SGX_ENCLAVE_SIGNER) sign -key $(Enclave_Key) -enclave $^ -out $@ -config $(Enclave_Config)
	@echo "SIGN => $@"

$(Test_Enclaves): %.so: %.o Enclave_t.o
	$(CC) $^ -o $@ $(Link_Flags)
	@echo "LINK => $@"

$(Test_Objs): %.o: %.c
	@$(CC) $(C_Flags) -c $< -o $@
	@echo "CC <= $@"

$(EDL_Gen_Files): $(SGX_EDGER8R) ../src/Enclave.edl
	@$(SGX_EDGER8R) --trusted ../src/Enclave.edl --search-path $(SGX_SDK)/include --search-path ../deps/rust-sgx-sdk/edl/
	@echo "GEN  => $(EDL_Gen_Files)"


.PHONY: compiler-rt
compiler-rt:
	@$(MAKE) --no-print-directory -C ../deps/rust-sgx-sdk/compiler-rt/ 2> /dev/null


#
# Run tests
#
.PHONY: test
test: $(Test_Targets)

.PHONY: (Test_Targets)
$(Test_Targets): test-%: %.signed.so
	./../src/pal/pal $^


.PHONY: clean
clean:
	@-$(RM) $(Test_Signed_Enclaves) $(Test_Enclaves) $(Test_Objs) $(EDL_Gen_Files)
