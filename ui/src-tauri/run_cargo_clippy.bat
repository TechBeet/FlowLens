@echo off
set PATH=C:\Users\21306\.cargo\bin;%PATH%
cd /d d:\github\FlowLens\ui\src-tauri
cargo clippy --lib -p flowlens-tauri -- -D warnings > cargo_clippy_out.txt 2>&1
echo EXIT: %ERRORLEVEL% >> cargo_clippy_out.txt