@echo off

for /f %%i in ('cargo --version') do set CARGO_VER=%%i
set CARGO_CMD=cargo +nightly
if not x%CARGO_VER:nightly=%==x%str1% set CARGO_CMD=cargo

set CARGO_INCREMENTAL=0
set RUSTFLAGS="-C link-arg=--export-table"
%CARGO_CMD% build --target=wasm32-unknown-unknown --release

for %%a in (node_runtime) do wasm-gc target\wasm32-unknown-unknown\release\%%a.wasm target\wasm32-unknown-unknown\release\totem-meccano.compact.wasm
