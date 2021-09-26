CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
XGBOOST_TRACKER := $(realpath $(CUR_DIR)/xgboost_src/dmlc-core/tracker/)
BIN := xgboost
DATA := data
TCS_NUM := $(shell expr $(shell nproc) \* 4)

.PHONY: all test test-native test-local-cluster clean

all: occlum_workspace

$(BIN):
	@cp -f xgboost_src/$(BIN) .

$(DATA):
	@rm -rf $(DATA) && mkdir -p $(DATA)
	@cp xgboost_src/demo/binary_classification/agaricus.txt.test $(DATA)
	@cp xgboost_src/demo/binary_classification/agaricus.txt.train $(DATA)
	@cp xgboost_src/demo/binary_classification/mushroom.conf $(DATA)
	@sed -i 's/agaricus/.\/data\/agaricus/g' $(DATA)/mushroom.conf
	@sed -i 's/num_round = 2/num_round = 10/g' $(DATA)/mushroom.conf
	@sed -i '$$a\model_out = "./host/result.model"' $(DATA)/mushroom.conf
	@sed -i '$$a\verbosity = 2' $(DATA)/mushroom.conf
	@sed -i '$$a\nthread = 2' $(DATA)/mushroom.conf

occlum_workspace: $(BIN) $(DATA)
	@rm -rf occlum_workspace && mkdir occlum_workspace
	@cd occlum_workspace && \
	occlum init && \
	jq '.process.default_mmap_size = "96MB" | .resource_limits.max_num_of_threads = $(TCS_NUM)' Occlum.json > temp_Occlum.json && \
	mv temp_Occlum.json Occlum.json && \
	copy_bom -f ../xgboost.yaml --root image --include-dir /opt/occlum/etc/template && \
	occlum build

test: occlum_workspace
	@./run_xgboost.sh

test-native: $(BIN) $(DATA)
	@rm -rf host
	@mkdir host
	@./$(BIN) ./$(DATA)/mushroom.conf

test-local-cluster: occlum_workspace
	@python3 $(XGBOOST_TRACKER)/dmlc-submit --cluster=local --num-workers=2 --log-level=DEBUG ./run_xgboost.sh

clean:
	@rm -rf $(BIN) $(DATA) tmp_* host occlum_workspace
