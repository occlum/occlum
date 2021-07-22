.PHONY: build install
build: generate_bom copy_bom update_bom
	cargo build --release
install: 
	cargo install --path generate_bom && cargo install --path copy_bom && cargo install --path update_bom
clean: 
	cargo clean
