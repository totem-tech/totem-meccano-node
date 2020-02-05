Set-Location Env:

$Env:RUST_BACKTRACE = "FULL"

$Env:CARGO_INCREMENTAL = 0
$Env:RUSTFLAGS = "-C link-arg=--export-table"
$cmd = wasm-gc target/wasm32-unknown-unknown/release/runtime_test.wasm target/wasm32-unknown-unknown/release/runtime_test.compact.wasm
invoke-expression -Command $cmd
