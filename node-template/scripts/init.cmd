echo off

echo "*** Initializing WASM build environment"

IF [%CI_PROJECT_NAME%] == [] (
    rustup update nightly
    rustup update stable
)

rustup target add wasm32-unknown-unknown --toolchain nightly
rem Install wasm-gc. It's useful for stripping slimming down wasm binaries.
wasm-gc | cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force
