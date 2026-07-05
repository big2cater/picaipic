@echo off
setlocal
cd /d "%~dp0"

if exist ".local.env" (
  for /f "usebackq eol=# tokens=1,* delims==" %%A in (".local.env") do (
    if not "%%A"=="" set "%%A=%%B"
  )
)

if "%NAFNET_SOURCE_DIR%"=="" (
  set "NAFNET_SOURCE_DIR=%~dp0models\nafnet"
)

if not "%PICAIPIC_PLUGIN_PYTHON%"=="" if exist "%PICAIPIC_PLUGIN_PYTHON%" (
  set "PICAIPIC_NAFNET_PYTHON=%PICAIPIC_PLUGIN_PYTHON%"
  goto :run
)

if not "%PICAIPIC_NAFNET_PYTHON%"=="" if exist "%PICAIPIC_NAFNET_PYTHON%" goto :run

if not "%PICAIPIC_PLUGIN_ENV_DIR%"=="" if exist "%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe" (
  set "PICAIPIC_NAFNET_PYTHON=%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe"
  goto :run
)

if exist ".venv\Scripts\python.exe" (
  set "PICAIPIC_NAFNET_PYTHON=.venv\Scripts\python.exe"
  goto :run
)

if exist ".venv-rocm\Scripts\python.exe" (
  set "PICAIPIC_NAFNET_PYTHON=.venv-rocm\Scripts\python.exe"
  goto :run
)

if exist ".venv-cuda\Scripts\python.exe" (
  set "PICAIPIC_NAFNET_PYTHON=.venv-cuda\Scripts\python.exe"
  goto :run
)

if exist ".venv-cpu\Scripts\python.exe" (
  set "PICAIPIC_NAFNET_PYTHON=.venv-cpu\Scripts\python.exe"
  goto :run
)

py -3 backend\main.py
if errorlevel 1 python backend\main.py
exit /b %errorlevel%

:run
echo Using Python: %PICAIPIC_NAFNET_PYTHON%
echo Using NAFNet source: %NAFNET_SOURCE_DIR%
"%PICAIPIC_NAFNET_PYTHON%" backend\main.py
exit /b %errorlevel%
