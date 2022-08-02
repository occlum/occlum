CC ?= gcc
LD ?= ld
LIBPATH ?= /opt/occlum/toolchains/dcap_lib/glibc
INCPATH ?= /opt/occlum/toolchains/dcap_lib/inc

.PHONY: all clean

all: dcap_c_test

dcap_c_test: dcap_c_test.c
		$(CC) $^ -fPIE -pie -o $@ -L $(LIBPATH) -locclum_dcap -I /opt/intel/sgxsdk/include -I $(INCPATH)

clean:
		rm -rf dcap_c_test
