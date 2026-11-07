@echo off
cd /d d:\github\FlowLens\ui
npm run build > npm_build_out.txt 2>&1
echo EXIT: %ERRORLEVEL% >> npm_build_out.txt