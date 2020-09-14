# Use Golang and SQLite with Occlum

This project demonstrates how Occlum enables [Golang](https://golang.org) programs with [SQLite](https://www.sqlite.org/index.html) library calls running in SGX enclaves. The way to use a SQL (or SQL-like) database in Go is through the [database/sql](https://golang.org/pkg/database/sql/) package. The SQL package exposes an ideal set of generic APIs for a variety of SQL or SQL-like databases. The SQL package itself must be used in conjunction with a [database driver](https://github.com/golang/go/wiki/SQLDrivers) package.

The demo program is based on a widely used [sqlite3 driver](https://github.com/mattn/go-sqlite3) conforming to the built-in database/sql interface and passing the compatibility test suite at [https://github.com/bradfitz/go-sql-test](https://github.com/bradfitz/go-sql-test). You can run the Golang SQLite demo on Occlum via
```
./run_go_sqlite_demo.sh
```

The demo program adds a database source file golang_sql_driver.db, creates a table SqlDrivers, inserts a list of Golang SQL database driver records, queries all records, and prints them out.
