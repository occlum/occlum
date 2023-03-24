# Secret acquisition with AECS client in Occlum Init

In this demo, we will show how to acquire secrets in Occlum init by AECS client.

[AECS](https://github.com/SOFAEnclave/enclave-configuration-service) is a short name of **Attestation based Enclave Configuration Service**. Basically, part of its function is acting as a remote attestation based key management service. 

Occlum provides a way to embed the AECS client function in Occlum Init process by simply running `occlum new --init-ra aecs` to initiate an Occlum instance.

## Start a demo AECS server

A public docker image for AECS server is provided for test only. It's running in simulation mode and also has debug log messages. Start it like this:
```
git clone https://github.com/SOFAEnclave/enclave-configuration-service.git
cd enclave-configuration-service
./deployment/aecs_test.sh start  # stop command to stop it
```

Once successful, a demo AECS server is started locally. It holds secrets **secret-my-keypair** and **secret-my-aes256-key** for test purpose. The below client demo tries to acquire those two secrets in customized **Occlum Init** by predefined **init_ra_conf.json**.

## Build and Run the client demo

### Prerequisites

A valid PCCS service should be accessible in your environment. This demo is verified in Aliyun, thus `https://sgx-dcap-server.cn-shanghai.aliyuncs.com/sgx/certification/v3/` is used as the PCCS URL. And please also make sure the client demo can access the locally started AECS server. For example, 

* Start the Occlum develop container with host network
```
docker run --rm -it \
     --device /dev/sgx/enclave --device /dev/sgx/provision \
     --network host \
     occlum/occlum:latest-ubuntu20.04 bash
```

In the container, update the **pccs_url** in the file `/etc/sgx_default_qcnl.conf` with the valid address.

### Build the demo

Just run `build.sh`, it generates an Occlum instance with:
* Init with AECS client.
* `busybox` is added to act as the real application.

Note, a valid PCCS URL needs to be passed to the Occlum instance. In our case, Aliyun `https://sgx-dcap-server.cn-shanghai.aliyuncs.com/sgx/certification/v3/` is used. Also, to acquire the secrets, the secret name and saved path should be filled into the `init_ra_conf.json` before occlum build. Details please refer to the script [build.sh](./build.sh).

### Run the demo

The boot flow of the demo is as below.
```
init (get secrets and save to rootfs per the definition in init_ra_conf.json) --> busybox
```

Thus, a simple command as below will print the secret **secret-my-keypair** acquired in **init** process.
```
occlum run /bin/busybox cat /etc/saved_secret_rsa_keypair
```
