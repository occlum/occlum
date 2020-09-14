package main

import (
        "database/sql"
        "fmt"
        _ "github.com/mattn/go-sqlite3"
        "os"
        "time"
)

type GolangDbDriver struct {
        dbName          string
        pkgLink         string
        testSuite       string
}

var DbDriverList = []GolangDbDriver {
        GolangDbDriver {
                dbName: "Apache Ignite/GridGain",
                pkgLink: "https://github.com/amsokol/ignite-go-client",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "Amazon AWS Athena",
                pkgLink: "https://github.com/uber/athenadriver",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "Google Cloud Spanner",
                pkgLink: "https://github.com/rakyll/go-sql-driver-spanner",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "MS SQL Server (pure go)",
                pkgLink: "https://github.com/denisenkom/go-mssqldb",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "MySQL",
                pkgLink: "https://github.com/go-sql-driver/mysql/",
                testSuite: "pass the compatibility test suite at https://github.com/bradfitz/go-sql-test",
        },
        GolangDbDriver {
                dbName: "MySQL",
                pkgLink: "https://github.com/siddontang/go-mysql/",
                testSuite: "pass the compatibility test suite but are not currently included in it",
        },
        GolangDbDriver {
                dbName: "Oracle (uses cgo)",
                pkgLink: "https://github.com/mattn/go-oci8",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "Postgres (pure Go)",
                pkgLink: "https://github.com/lib/pq",
                testSuite: "pass the compatibility test suite at https://github.com/bradfitz/go-sql-test",
        },
        GolangDbDriver {
                dbName: "Postgres (uses cgo)",
                pkgLink: "https://github.com/jbarham/gopgsqldriver",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "Snowflake (pure Go)",
                pkgLink: "https://github.com/snowflakedb/gosnowflake",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "SQLite (uses cgo)",
                pkgLink: "https://github.com/mattn/go-sqlite3",
                testSuite: "pass the compatibility test suite at https://github.com/bradfitz/go-sql-test",
        },
        GolangDbDriver {
                dbName: "SQLite (uses cgo)",
                pkgLink: "https://github.com/gwenn/gosqlite",
                testSuite: "Unknown",
        },
        GolangDbDriver {
                dbName: "Apache Hive",
                pkgLink: "https://github.com/sql-machine-learning/gohive",
                testSuite: "Unknown",
        },
}

func main() {
        previousTime := time.Now()
        fmt.Printf("Starting the Golang SQLite demo: at %s\n", previousTime.Format("2006-01-02T15:04:05.999999999Z07:00"))

        dbSourceName := "golang_sql_driver.db"
        os.Remove(dbSourceName)

        dbDriverName := "sqlite3"
        db, err := sql.Open(dbDriverName, dbSourceName)
        if err != nil {
                fmt.Println("Failed to open the database:", err)
        }
        defer db.Close()

        if err = db.Ping(); err != nil {
                fmt.Println("Failed to establish a connection to the database:", err)
        }

        tx, err := db.Begin()
        if err != nil {
                fmt.Println("Failed to start a database transaction:", err)
        }

        sqlStatement := `
        create table if not exists SqlDrivers (dbName varchar(255), pkgLink varchar(255), testSuite varchar(255));
        delete from SqlDrivers;
        `
        _, err = db.Exec(sqlStatement)
        if err != nil {
                fmt.Println("Failed to create table:", sqlStatement, err)
                return
        }

        statement, err := tx.Prepare("insert into SqlDrivers(dbName, pkgLink, testSuite) values(?, ?, ?)")
        if err != nil {
                fmt.Println("Failed to prepare SQL statements:", err)
        }
        defer statement.Close()
        for _, entry := range DbDriverList {
                _, err = statement.Exec(entry.dbName, entry.pkgLink, entry.testSuite)
                if err != nil {
                        fmt.Println("Failed to prepare SQL statements:", err)
                }
        }

        tx.Commit()

        rows, err := db.Query("select dbName, pkgLink, testSuite from SqlDrivers")
        if err != nil {
                fmt.Println("Failed to query the database:", err)
        }
        defer rows.Close()
        for rows.Next() {
                var dbName sql.NullString
                var pkgLink sql.NullString
                var testSuite sql.NullString
                err = rows.Scan(&dbName, &pkgLink, &testSuite)
                if err != nil {
                        fmt.Println("Failed to Scan the query results:", err)
                }
                fmt.Printf("dbName=%v;\n  pkgLink=%v;\n  testSuite=%v\n", dbName.String, pkgLink.String, testSuite.String)
        }
        err = rows.Err()
        if err != nil {
                fmt.Println("Error was encountered during query result iteration:", err)
        }

        _, err = db.Exec("delete from SqlDrivers")
        if err != nil {
                fmt.Println("Failed to delete all records in the database:", err)
        }

        currentTime := time.Now()
        fmt.Printf("Total running time: %f Seconds\n", currentTime.Sub(previousTime).Seconds())

        os.Remove(dbSourceName)
}

