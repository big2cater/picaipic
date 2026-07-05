@echo off
setlocal
cd /d "%~dp0"

if exist ".local.env" (
  for /f "usebackq eol=# tokens=1,* delims==" %%A in (".local.env") do (
    if not "%%A"=="" set "%%A=%%B"
  )
)

if "%SALUT_SOURCE_DIR%"=="" if exist "%~dp0backend\engine" (
  set "SALUT_SOURCE_DIR=%~dp0backend"
)

if not "%PICAIPIC_PLUGIN_RUNTIME_ROOT%"=="" if "%SALUT_SOURCE_DIR%"=="" if exist "%PICAIPIC_PLUGIN_RUNTIME_ROOT%\engine" (
  set "SALUT_SOURCE_DIR=%PICAIPIC_PLUGIN_RUNTIME_ROOT%"
)

if not "%PICAIPIC_PLUGIN_RUNTIME_DIR%"=="" if "%SALUT_SOURCE_DIR%"=="" if exist "%PICAIPIC_PLUGIN_RUNTIME_DIR%\engine" (
  set "SALUT_SOURCE_DIR=%PICAIPIC_PLUGIN_RUNTIME_DIR%"
)

if not "%PICAIPIC_PLUGIN_PYTHON%"=="" if exist "%PICAIPIC_PLUGIN_PYTHON%" (
  set "PICAIPIC_SALUT_PYTHON=%PICAIPIC_PLUGIN_PYTHON%"
  goto :run
)

if not "%PICAIPIC_SALUT_PYTHON%"=="" if exist "%PICAIPIC_SALUT_PYTHON%" goto :run

if not "%PICAIPIC_PLUGIN_ENV_DIR%"=="" if exist "%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe"
  goto :run
)

if exist ".venv\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv\Scripts\python.exe"
  goto :run
)

if exist ".venv-rocm\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv-rocm\Scripts\python.exe"
  goto :run
)

if exist ".venv-cuda\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv-cuda\Scripts\python.exe"
  goto :run
)

if exist ".venv-directml\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv-directml\Scripts\python.exe"
  goto :run
)

if exist ".venv-cpu\Scripts\python.exe" (
  set "PICAIPIC_SALUT_PYTHON=.venv-cpu\Scripts\python.exe"
  goto :run
)

py -3 backend\main.py
if errorlevel 1 python backend\main.py
exit /b %errorlevel%

:run
echo Using Python: %PICAIPIC_SALUT_PYTHON%
echo Using SA-LUT source: %SALUT_SOURCE_DIR%
"%PICAIPIC_SALUT_PYTHON%" backend\main.py
exit /b %errorlevel%
