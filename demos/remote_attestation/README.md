# SGX Remote Attestation Demo

This project demonstrates how to do remote attestation on Occlum.

In a nutshell, Occlum provides SGX capabilities to user apps through ioctls on a special device (`/dev/sgx`). To hide the low-level details of ioctls from user apps, a user-friendly, remote attestation library is provided in this demo.

**Prerequisites.** This demo needs to access Intel Attestation Service (IAS). To do this, a developer needs to contact Intel to obtain a Service Provider ID (SPID) and the associated Service Provider certificate. The certificate and key files should be put into `conf/certs`, and configure the SPID and paths of the certificate and key files in `conf/ra_config.example.json`.

**Step 1.** Build this demo
```
download_and_build.sh
```

**Step 2.** Run this demo on Occlum
```
run_on_occlum.sh
```

