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

SUFFIX="-h 127.0.0.1 -P 3306"
echo -e "${GREEN}Run mysql client on Occlum${NC}"

# Client on Occlum (TCP/IP)
occlum exec /bin/${MYSQLADMIN} version ${SUFFIX}

occlum exec /bin/${MYSQLSHOW} ${SUFFIX}

occlum exec /bin/${MYSQL} -e "SELECT User, Host, plugin FROM mysql.user" ${SUFFIX}

echo -e "${GREEN}Run mysql client on host${NC}"

# Client on host (TCP/IP)
/usr/local/mysql/bin/${MYSQLADMIN} version ${SUFFIX}

/usr/local/mysql/bin/${MYSQLSHOW} ${SUFFIX}

/usr/local/mysql/bin/${MYSQL} -e "SELECT User, Host, plugin FROM mysql.user" ${SUFFIX}

occlum stop

popd
