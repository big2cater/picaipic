@echo off
setlocal
cd /d "%~dp0"

if not "%PICAIPIC_PLUGIN_ENV_DIR%"=="" if exist "%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe"
  goto :run
)

if exist ".venv-rocm\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv-rocm\Scripts\python.exe"
  goto :run
)

py -3 backend\main.py
exit /b %errorlevel%

:run
echo Using Python: %PICAIPIC_SALUT_PYTHON%
"%PICAIPIC_SALUT_PYTHON%" backend\main.py
exit /b %errorlevel%
