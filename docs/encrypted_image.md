# The Encrypted FS Image
Occlum has supported using an encrypted FS image, which is encrypted by a user-provided key, to run apps inside the enclave. The confidentiality and integrity of user's files and libraries are both protected with it.

## How to use
To generate the encrypted FS image, user must give the `--image-key <key_path>` flag in the `occlum build` command (If the flag is not given, the secure FS image will be integrity protected only). The `<key_path>` refers to a file consisting of a 128-bit key and the user can generate it via the `occlum gen-image-key <key_path>` command.
