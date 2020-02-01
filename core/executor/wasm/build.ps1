Set-Location Env:

$CARGO_VER = invoke-expression -Command "cargo --version"
$CARGO_CMD = "cargo"
if ($CARGO_VER.Contains('nightly')) {
    $CARGO_CMD = "cargo +nightly"
}

$Env:CARGO_INCREMENTAL = 0
$Env:RUSTFLAGS = "-C link-arg=--export-table"
$cmd = "$CARGO_CMD build --target=wasm32-unknown-unknown --release"

$test = invoke-expression -Command $cmd
foreach ($i in $test) {
    invoke-expression -Command wasm-gc target/wasm32-unknown-unknown/release/runtime_$i.wasm target/wasm32-unknown-unknown/release/runtime_$i.compact.wasm
}
