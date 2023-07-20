# Run SWTPM on Occlum

[`SWTPM`](https://github.com/stefanberger/swtpm) is a widely used open-source software-based Trusted Platform  Module (TPM) emulator based on [`Libtpms`](https://github.com/stefanberger/libtpms). This project demonstrates how SWTPM can be used in SGX enclave using Occlum.

Step 1: Download and install SWTPM
```
./install_swtpm.sh
```
This command downloads Libtpms and SWTPM source code and builds from it.
When completed, all SWTPM related binaries and tools are installed.

Step 2: Run SWTPM
```
./run_swtpm.sh
```
This command initializes and runs the SWTPM in SGX.

When completed, the server starts to wait for TPM Software Stack (TSS). SWTPM is compatible with all type of TSS. For more information on TSS, check [`IBM's TPM2.0 TSS`](https://sourceforge.net/p/ibmtpm20tss/tss/ci/master/tree/) or [`TCG TPM2 TSS`](https://github.com/tpm2-software/tpm2-tss).


(Optional) Step 3: Test with [`IBM's TPM2.0 TSS`](https://sourceforge.net/p/ibmtpm20tss/tss/ci/master/tree/)
```
./run_client.sh
```
This command first install TSS and specifies the TPM ports. Next, it starts the TPM and runs getrandom function to create a random number of 128 Bytes. The output is similar to as given below.

 ```
 d5 7b b6 98 ce 93 c1 55 66 0d 90 d0 24 ae fc 3a 
 89 09 00 a7 ea d3 ca c8 4d 40 46 60 53 21 00 0a 
 eb a7 eb ef 13 3e 0a de df 29 85 8c 50 34 c0 0c 
 2a 9e 74 e4 50 65 c2 30 16 eb e8 e3 a2 74 a9 7c 
 84 06 7c 0f 4e 10 1c 0c 80 fb a7 1c 0b ba 13 d7 
 de 25 e0 44 2f 22 75 76 70 87 e0 a3 c5 bb 28 5c 
 df 26 a5 92 48 e2 3a e5 77 ce 76 df 76 84 3a 6a 
 b7 97 33 94 8d 57 2e 90 b5 61 89 cb 62 ed ce 09
```




