CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../../)
OPENSSL ?= $(CUR_DIR)/../deps/openssl
SGX_SDK ?= /opt/intel/sgxsdk
OBJS := session.o proc_msg.o
CC := occlum-gcc
CFLAGS := -fPIC
SOFLAGS := -shared $(CFLAGS)
OPENSSL := -L$(OPENSSL) -lcrypto
ECDH := -L$(CUR_DIR)/../DiffieHellmanLibrary -lecdh
INCLUDE_PATH := -I$(SGX_SDK)/include -I../Include

responder : $(OBJS)
	$(CC)  responder.c $(OBJS) $(OPENSSL) $(ECDH) $(INCLUDE_PATH) -o responder

session.o : session.c
	$(CC)  -c session.c $(CFLAGS) $(INCLUDE_PATH) -o session.o

proc_msg.o : proc_msg.c
	$(CC)  -c proc_msg.c $(CFLAGS) $(INCLUDE_PATH) -o proc_msg.o

clean:
	@rm -rf *.o  responder
