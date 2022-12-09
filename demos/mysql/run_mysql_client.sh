#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

MYSQL=mysql
MYSQLSHOW=mysqlshow
MYSQLADMIN=mysqladmin

# Need to wait until server is ready
echo -e "${GREEN}Need to wait mysql server to start${NC}"

pushd occlum_instance

echo -e "${GREEN}Run mysql client on Occlum${NC}"

# Use unix domain socket
occlum exec /bin/${MYSQLADMIN} version

occlum exec /bin/${MYSQLSHOW}

occlum exec /bin/${MYSQL} -e "SELECT User, Host, plugin FROM mysql.user"

echo -e "${GREEN}Run mysql client on host${NC}"

# Use TCP/IP
/usr/local/mysql/bin/${MYSQLSHOW} -h 127.0.0.1 -P 3306

occlum stop

popd
