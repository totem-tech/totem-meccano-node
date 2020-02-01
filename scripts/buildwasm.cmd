@echo off
set "ROOT=%cd%"
echo %ROOT%
echo *** Building wasm binaries in %1
cd %ROOT%\%1
echo building...
build.cmd
cd %ROOT%