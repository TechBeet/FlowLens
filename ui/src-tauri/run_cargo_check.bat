@echo off
set PATH=C:\Users\21306\.cargo\bin;%PATH%
cd /d d:\github\FlowLens\ui\src-tauri
cargo check > cargo_check_out.txt 2>&1
echo EXIT: %ERRORLEVEL% >> cargo_check_out.txt