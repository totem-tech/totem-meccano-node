# PowerShell port of build scripts for Totem
Set-Location Env:

$SRCS=@(
	"core\executor\wasm",
	"node\runtime\wasm",
	"node-template\runtime\wasm",
	"core\test-runtime\wasm"
)
$NODETEMPLATE=@(
	"node_runtime"
)

$CARGO_VER = invoke-expression -Command "cargo --version"
$CARGO_CMD = "cargo"
if ($CARGO_VER.Contains('nightly')) {
    $CARGO_CMD = "cargo +nightly"
}

$PROJECT_ROOT = ''
if ($MyInvocation.MyCommand.Path) {
    $PROJECT_ROOT = Split-Path $MyInvocation.MyCommand.Path
} else {
    $PROJECT_ROOT = $pwd -replace '^\S+::',''
}

if ($PROJECT_ROOT.EndsWith('scripts')) {
    $PROJECT_ROOT = $PROJECT_ROOT.Substring(0, $PROJECT_ROOT.Length - 8)
}

foreach ($subdir in $SRCS) {
    echo "*** Building wasm binaries in $PROJECT_ROOT\$subdir"
    Set-Location -Path $PROJECT_ROOT\$subdir
    # set env variables
    $Env:CARGO_INCREMENTAL = 0
    $Env:RUSTFLAGS = "-C link-arg=--export-table"
    $cmd = "$CARGO_CMD build --target=wasm32-unknown-unknown --release"
    # build
    $test = invoke-expression -Command $cmd
    foreach ($i in $NODETEMPLATE) {
        invoke-expression -Command wasm-gc target/wasm32-unknown-unknown/release/runtime_$i.wasm target/wasm32-unknown-unknown/release/runtime_$i.compact.wasm
    }
}
Set-Location -Path $PROJECT_ROOT