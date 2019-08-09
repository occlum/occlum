# protect-integrity

This is a command-line utility that protects the _integrity_ of a file using the _integrity-only_ mode of SGX Protected File System Library.

## Prerequesite

This integrity-only mode is provided by Occlum's fork of Intel SGX SDK, not available on vanilla Intel SGX SDK. So make sure that you have Occlum's fork of Intel SGX SDK installed.

## How to Build

To build the project, run the following command

    make

To test the project, run the following command

    make test

## How to Use

To protect an ordinary file, run the following command

    ./protect-integrity protect <ordinary_file>

which will generate a protected file named `<ordinary_file>.protected` in the current working directory. The content of `<ordinary_file>.protected` is the same as `<ordinary_file` but associated with (a tree of) 128-bit MACs to protect its integrity.

To show the content of a protected file, run the following command

    ./protect-integrity show <protected_file>

To show the (root) MAC of a protected file, run the following command

    ./protect-integrity show-mac <protected_file>

## Note

This utility is intended to be used in _trusted_ development environment, not _untrusted_ deployment environment.
