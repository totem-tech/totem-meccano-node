#!/usr/bin/env bash
set -e

if cargo --version | grep -q "nightly"; then
	CARGO_CMD="cargo"
else
	CARGO_CMD="cargo +nightly"
fi
CARGO_INCREMENTAL=0 RUSTFLAGS="-C link-arg=--export-table" $CARGO_CMD build --target=wasm32-unknown-unknown --release
for i in node_template_runtime_wasm
do
	# End of life for wasm-gc
	# wasm-gc target/wasm32-unknown-unknown/release/$i.wasm target/wasm32-unknown-unknown/release/totem-meccano-template.compact.wasm
	rm -f target/wasm32-unknown-unknown/release/totem-meccano-template.compact.wasm
	yes | cp -rf target/wasm32-unknown-unknown/release/$i.wasm target/wasm32-unknown-unknown/release/totem-meccano-template.compact.wasm
done
