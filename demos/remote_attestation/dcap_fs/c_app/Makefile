CC ?= gcc
LD ?= ld

.PHONY: all clean

all: dcap_fs

dcap_fs: dcap_fs.c
		$(CC) $^ -fPIE -pie -o $@ -I /opt/intel/sgxsdk/include

clean:
		rm -rf dcap_fs
