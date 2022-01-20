CC ?= gcc
LD ?= ld
LIBPATH ?= ../dcap_lib/target/debug

.PHONY: all clean

all: dcap_c_test

dcap_c_test: dcap_c_test.c
		$(CC) $^ -fPIE -pie -o $@ -L $(LIBPATH) -ldcap_quote -I /opt/intel/sgxsdk/include

clean:
		rm -rf dcap_c_test
