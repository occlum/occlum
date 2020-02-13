# Use SQLite in SGX with Occlum

This project demonstrates how Occlum enables [SQLite](https://www.sqlite.org) in SGX enclaves.

Step 1: Download SQLite and build the demo program
```
./download_and_build_sqlite.sh
```
When completed, the demo program (i.e., `sqlite_demo`) is generated.

Step 2: Run the SQLite demo program inside SGX enclave with Occlum
```
./run_sqlite_on_occlum.sh
```

Step 3 (Optional): Run the SQLite demo program in Linux
```
./sqlite_demo <database> <sql-statement>
```
