@echo off

for /f %%i in ('cargo --version') do set CARGO_VER=%%i

set CARGO_CMD="cargo +nightly"
if not x%CARGO_VER:nightly=%==x%str1% set CARGO_CMD="cargo"
echo %CARGO_CMD%

rem CARGO_INCREMENTAL=0 RUSTFLAGS="-C link-arg=--export-table" $CARGO_CMD build --target=wasm32-unknown-unknown --release
rem for i in node_runtime
rem do
rem 	wasm-gc target/wasm32-unknown-unknown/release/$i.wasm target/wasm32-unknown-unknown/release/totem-meccano.compact.wasm
rem done
