@echo off

rem This script assumes that all pre-requisites are installed.

set "PROJECT_ROOT=%cd%"
set CARGO_INCREMENTAL=0

rem for /f "usebackq eol=: delims=" %%F in ("%PROJECT_ROOT%\scripts\dirlist.txt") do .\scripts\buildwasm.cmd "%%F"
.\scripts\buildwasm.cmd core\executor\wasm