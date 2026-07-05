@echo off
setlocal

rem PicAiPic terminates the tracked child process after this script exits.
rem Also clean up stale backend processes from older/dev runs of this plugin.
if not defined PICAIPIC_PLUGIN_PORT set "PICAIPIC_PLUGIN_PORT=8011"

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0backend\stop_plugin.ps1"

exit /b 0
