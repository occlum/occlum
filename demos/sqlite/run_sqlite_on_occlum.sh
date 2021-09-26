#!/bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/sqlite.yaml

export DEMO=sqlite_demo
export SPEEDTEST=speedtest1

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

if [ ! -e $SPEEDTEST ];then
    echo "Error: cannot stat '$SPEEDTEST'"
    echo "Please see README and build the $SPEEDTEST"
    exit 1
fi

# 1. Init Occlum Workspace
rm -rf occlum_instance && occlum new occlum_instance
cd occlum_instance

# 2. Copy files into Occlum Workspace and build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run the demo
occlum run /bin/$DEMO "$SQL_DB" "$SQL_STMT"
occlum run /bin/$SPEEDTEST --memdb --stats --size 100
