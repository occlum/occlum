# The Encrypted FS Image
Occlum has supported using an encrypted FS image, which is encrypted by a user-provided key, to run apps inside the enclave. The confidentiality and integrity of user's files and libraries are both protected with it.

## How to use
To generate the encrypted FS image, user must give the `--image-key <key_path>` flag in the `occlum build` command (If the flag is not given, the secure FS image will be integrity protected only).

The `<key_path>` refers to a file consisting of a 128-bit key and the user can generate it via the `occlum gen-image-key <key_path>` command.

After generating the encrypted FS image, the [init](https://github.com/occlum/occlum/tree/master/tools/init) process is responsible for mounting the encrypted FS image as the rootfs for the user's Application. Usually the key should be acquired through RA or LA, please take the [init_ra](https://github.com/occlum/occlum/tree/master/demos/remote_attestation/init_ra_flow/init_ra) as an example to use this feature in real world.
