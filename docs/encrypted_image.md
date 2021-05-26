# The Encrypted FS Image
Since 0.22.0, Occlum has supported using an encrypted FS image, which is encrypted by a user-provided key, to run apps inside the enclave. The confidentiality and integrity of user's files and libraries are both protected with it.

## How to use
To generate the encrypted FS image, user must give the `--image-key <key_path>` flag in the `occlum build` command (If the flag is not given, the secure FS image will be integrity protected only). The `<key_path>` refers to a file consisting of a 128-bit key and the user can generate it via the `occlum gen-image-key <key_path>` command.

If user also gives the `--buildin-image-key` flag in the `occlum build` command, the key file will be packaged into the initfs after building the Occlum instance. The initfs is an integrity-only protected FS image, which is used by the [init](../tools/init) program to mount the encrypted FS image as the rootfs for the user's apps.

To use this feature in real world, user must ***NOT*** give the `--buildin-image-key` flag in the `occlum build` command, and modify the [init](../tools/init) program to acquire the key through RA or LA. We would remove the `--buildin-image-key` flag when the "init-RA" demo is ready.
