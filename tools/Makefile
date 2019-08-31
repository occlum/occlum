.PHONY: all clean

all:
	@mkdir -p ../build/bin/
	@ln -s -f ../../tools/occlum ../build/bin/occlum
	@ln -s -f ../../tools/occlum-build-enclave ../build/bin/occlum-build-enclave
	@ln -s -f ../../tools/occlum-gen-default-occlum-json ../build/bin/occlum-gen-default-occlum-json
	@$(MAKE) --no-print-directory -C protect-integrity

clean:
	@$(MAKE) --no-print-directory -C protect-integrity clean
