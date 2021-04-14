# Run HashiCorp Vault with Occlum

This project demonstrates how Occlum enables [HashiCorp Vault](https://github.com/hashicorp/vault) in SGX enclaves.

Step 1: Download Vault source code and build the Vault executable
```
./prepare_vault.sh
```
Once completed, the resulting Vault source code can be found in the `source_code` directory with the built binary located in `./source_code/bin`.

Step 2: Run Vault server in `dev` mode with a custom initial root token inside SGX enclave with Occlum
```
./run_occlum_vault_server.sh
```

Step 3: In another terminal, run Vault `kv` CLI for interacting with Vault's key/value secrets engine
```
./run_occlum_vault_test.sh
```
