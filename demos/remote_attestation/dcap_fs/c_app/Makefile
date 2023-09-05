CC ?= gcc
LD ?= ld
LIBPATH ?= /opt/occlum/toolchains/dcap_lib/glibc
INCPATH ?= /opt/occlum/toolchains/dcap_lib/inc

.PHONY: all clean

all: dcap_fs

dcap_fs: dcap_fs.c
		$(CC) $^ -fPIE -pie -o $@ -L $(LIBPATH) -locclum_dcap -I /opt/intel/sgxsdk/include -I $(INCPATH)

clean:
		rm -rf dcap_fs
