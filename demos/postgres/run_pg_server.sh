#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'


pushd occlum_instance

# Start Occlum instance
occlum start

echo -e "${GREEN}Run pg server on Occlum${NC}"

occlum exec /usr/local/pgsql/bin/pg_ctl -D /usr/local/pgsql/data -l /host/logfile start

/usr/local/pgsql/bin/createdb test -h localhost
# /usr/local/pgsql/bin/psql test -h localhost

echo -e "${GREEN}Run pgbench on Occlum via network socket${NC}"

/usr/local/pgsql/bin/pgbench -i -h localhost test
/usr/local/pgsql/bin/pgbench -h localhost -c 10 -t 1000 test

echo -e "${GREEN}Stop pg server on Occlum${NC}"
occlum stop

popd
