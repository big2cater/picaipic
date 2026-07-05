@echo off
setlocal
cd /d "%~dp0"

if "%PICAIPIC_PLUGIN_ENV_DIR%"=="" (
  set "PICAIPIC_PLUGIN_ENV_DIR=.venv"
)
if "%PICAIPIC_PLUGIN_REQUIREMENTS%"=="" (
  set "PICAIPIC_PLUGIN_REQUIREMENTS=backend\requirements.txt"
)
if "%PICAIPIC_PLUGIN_REQUIREMENTS_PATH%"=="" (
  set "PICAIPIC_PLUGIN_REQUIREMENTS_PATH=%CD%\%PICAIPIC_PLUGIN_REQUIREMENTS%"
)

echo PicAiPic NAFNet setup
echo   profile: %PICAIPIC_PLUGIN_PROFILE_ID%
echo   backend: %PICAIPIC_PLUGIN_BACKEND%
echo   runtime scope: %PICAIPIC_PLUGIN_RUNTIME_SCOPE%
echo   runtime id: %PICAIPIC_PLUGIN_RUNTIME_ID%
echo   env: %PICAIPIC_PLUGIN_ENV_DIR%
echo   requirements: %PICAIPIC_PLUGIN_REQUIREMENTS_PATH%
echo.

if /i "%PICAIPIC_PLUGIN_RUNTIME_SCOPE%"=="external" (
  echo External runtime binding selected.
  echo   python: %PICAIPIC_PLUGIN_PYTHON%
  echo   root: %PICAIPIC_PLUGIN_RUNTIME_ROOT%
  if not exist "%PICAIPIC_PLUGIN_PYTHON%" (
    echo External Python was not found.
    exit /b 1
  )
  "%PICAIPIC_PLUGIN_PYTHON%" -m pip --version
  if errorlevel 1 exit /b %errorlevel%
  echo External runtime is present. Run Smoke in PicAiPic before using this profile.
  exit /b 0
)

set "PICAIPIC_PLUGIN_ENV_PYTHON=%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\python.exe"
set "PICAIPIC_PLUGIN_ENV_ACTIVATE=%PICAIPIC_PLUGIN_ENV_DIR%\Scripts\activate.bat"

if not exist "%PICAIPIC_PLUGIN_ENV_PYTHON%" (
  echo Creating virtual environment: %PICAIPIC_PLUGIN_ENV_DIR%
  py -3 -m venv "%PICAIPIC_PLUGIN_ENV_DIR%"
  if errorlevel 1 python -m venv "%PICAIPIC_PLUGIN_ENV_DIR%"
  if errorlevel 1 exit /b 1
)

if not exist "%PICAIPIC_PLUGIN_ENV_PYTHON%" (
  echo Virtual environment Python was not created: %PICAIPIC_PLUGIN_ENV_PYTHON%
  exit /b 1
)

if not exist "%PICAIPIC_PLUGIN_ENV_ACTIVATE%" (
  echo Virtual environment activation script was not found: %PICAIPIC_PLUGIN_ENV_ACTIVATE%
  exit /b 1
)

call "%PICAIPIC_PLUGIN_ENV_ACTIVATE%"
if errorlevel 1 exit /b 1

python -m pip install --upgrade pip
if errorlevel 1 exit /b 1

python -m pip install -r "%PICAIPIC_PLUGIN_REQUIREMENTS_PATH%"
if errorlevel 1 exit /b 1

python backend\verify_setup.py
if errorlevel 1 exit /b 1

echo.
echo NAFNet plugin runtime dependencies are installed and verified.
echo Run Smoke in PicAiPic before using this profile.
exit /b 0
