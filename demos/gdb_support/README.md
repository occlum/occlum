# GDB support for apps running upon Occlum

## GDB support

We modified the GDB SGX plugin to support debugging the apps running upon Occlum. After Occlum loaded app's executable file and its dependencies(e.g. libc) from the secure FS, it notifies GDB to load the symbols from elf file to the correct address allocated by Occlum, so GDB can find all the symbols of the running app.

Currently, GDB cannot unload the app's symbols after running by Occlum, `gdb attach` command is not supported, and the backtrace cannot link the app with Occlum, we will support these features in the later version.

## How to use

Step 1: Build the sample app with debugging symbols, add `-g` flags generally
```
./build_sample_with_symbols.sh
```

Step 2: Debug the sample app running on Occlum via `occlum gdb`
```
./gdb_sample_on_occlum.sh
```
When completed, shell changes to GDB.

Step 3: Type `run` in the GDB shell to run the sample app
```
(gdb) run
```
GDB will stop at the `divide_by_zero` function because of the arithmetic exception.
