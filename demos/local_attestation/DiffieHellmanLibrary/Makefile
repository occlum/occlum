CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../../)
OPENSSL ?= $(CUR_DIR)/../deps/openssl
SGX_SDK ?= /opt/intel/sgxsdk

OBJS = sgx_ecc256.o  sgx_cmac128.o sgx_sha256_msg.o crypto_aes_gcm.o
CC = occlum-gcc
CXX = occlum-g++
CFLAGS =  -I$(OPENSSL)/include -L$(OPENSSL) -lcrypto -fPIC
SOFLAGS = -shared  $(CFLAGS)
CFLAGS += -I$(SGX_SDK)/include -Iinclude

ec_dh : $(OBJS)
	$(CXX) $(SOFLAGS) -o libecdh.so ec_dh.cpp  $(OBJS)

sgx_ecc256.o : sgx_ecc256.cpp
	$(CXX)  -c sgx_ecc256.cpp $(CFLAGS) -o sgx_ecc256.o

sgx_cmac128.o : sgx_cmac128.cpp
	$(CXX)  -c sgx_cmac128.cpp $(CFLAGS) -o sgx_cmac128.o

sgx_sha256_msg.o : sgx_sha256_msg.cpp
	$(CXX)  -c sgx_sha256_msg.cpp $(CFLAGS)  -o sgx_sha256_msg.o

crypto_aes_gcm.o : crypto_aes_gcm.cpp
	$(CXX)  -c crypto_aes_gcm.cpp  $(CFLAGS)  -o crypto_aes_gcm.o

clean:
	@rm -rf *.o ec_dh *.so
