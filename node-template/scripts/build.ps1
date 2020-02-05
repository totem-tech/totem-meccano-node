# PowerShell port of build scripts for Totem
Set-Location Env:

$SRC=@(
	"runtime\wasm"
)
$NODETEMPLATE=@(
	"node_template_runtime_wasm"
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
$PROJECT_ROOT = [System.IO.Path]::GetDirectoryName($PROJECT_ROOT)

foreach ($subdir in $SRC) {
    echo "*** Building wasm binaries in $PROJECT_ROOT\$subdir"
    Set-Location -Path $PROJECT_ROOT\$subdir
    # set env variables
    $Env:CARGO_INCREMENTAL = 0
    $Env:RUSTFLAGS = "-C link-arg=--export-table"
    $cmd = "$CARGO_CMD build --target=wasm32-unknown-unknown --release"
    # build
    $test = invoke-expression -Command $cmd
    foreach ($i in $NODETEMPLATE) {
        $subcmd = "wasm-gc target\wasm32-unknown-unknown\release\runtime_$i.wasm target\wasm32-unknown-unknown\release\runtime_$i.compact.wasm"
        echo $subcmd
        invoke-expression -Command $subcmd
    }
}
Set-Location -Path $PROJECT_ROOT