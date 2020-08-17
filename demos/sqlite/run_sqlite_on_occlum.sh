#!/bin/bash
set -e

DEMO=sqlite_demo
SQL_DB=/root/company.db
SQL_STMT="CREATE TABLE COMPANY ( \
    ID INT PRIMARY KEY NOT NULL, \
    NAME TEXT NOT NULL, \
    AGE INT NOT NULL, \
    ADDRESS CHAR(50), \
    SALARY REAL ); \
    INSERT INTO COMPANY VALUES ( 1, 'Kris', 27, 'California', 16000.00 ); \
    SELECT * FROM COMPANY;"

if [ ! -e $DEMO ];then
    echo "Error: cannot stat '$DEMO'"
    echo "Please see README and build the $DEMO"
    exit 1
fi

# 1. Init Occlum Workspace
rm -rf occlum_instance && mkdir occlum_instance
cd occlum_instance
occlum init

# 2. Copy files into Occlum Workspace and build
cp ../$DEMO image/bin
occlum build

# 3. Run the demo
occlum run /bin/$DEMO "$SQL_DB" "$SQL_STMT"
