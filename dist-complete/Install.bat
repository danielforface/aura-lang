@echo off
REM Aura v1.0 Installation Script (Windows Batch)
REM This script installs Aura to C:\Program Files\Aura

setlocal enabledelayedexpansion

echo.
echo ╔════════════════════════════════════════╗
echo ║  Aura v1.0 Quick Installation        ║
echo ╚════════════════════════════════════════╝
echo.

REM Check for admin rights
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo WARNING: This script should be run as Administrator
    echo Please run Command Prompt as Administrator and try again
    pause
    exit /b 1
)

REM Set installation path
set "INSTALL_PATH=%ProgramFiles%\Aura"

echo Installation Settings:
echo   Install Path: %INSTALL_PATH%
echo.

REM Create installation directory
echo Creating installation directory...
if not exist "%INSTALL_PATH%" (
    mkdir "%INSTALL_PATH%"
    echo   Created installation directory
) else (
    echo   Directory exists, updating...
)

REM Copy binaries
echo.
echo Copying binaries...
if exist "%cd%\bin" (
    xcopy "%cd%\bin" "%INSTALL_PATH%\bin" /E /I /Y >nul
    echo   Copied binaries
)

REM Copy libraries
echo Copying libraries...
if exist "%cd%\lib" (
    xcopy "%cd%\lib" "%INSTALL_PATH%\lib" /E /I /Y >nul
    echo   Copied libraries
)

REM Copy documentation
echo Copying documentation...
if exist "%cd%\docs" (
    xcopy "%cd%\docs" "%INSTALL_PATH%\docs" /E /I /Y >nul
    echo   Copied documentation
)

REM Copy other directories
echo Copying SDK and examples...
for %%D in (sdk examples apps config) do (
    if exist "%cd%\%%D" (
        xcopy "%cd%\%%D" "%INSTALL_PATH%\%%D" /E /I /Y >nul
        echo   Copied %%D
    )
)

REM Copy main documentation files
if exist "%cd%\README.md" xcopy "%cd%\README.md" "%INSTALL_PATH%" /Y >nul
if exist "%cd%\MANIFEST.md" xcopy "%cd%\MANIFEST.md" "%INSTALL_PATH%" /Y >nul

REM Add to PATH
echo.
echo Adding to system PATH...
setx PATH "%PATH%;%INSTALL_PATH%\bin" >nul
if %errorlevel% equ 0 (
    echo   Added to PATH
) else (
    echo   WARNING: Could not modify PATH
)

REM Verify installation
echo.
echo Verifying installation...
set "VALID=1"

if exist "%INSTALL_PATH%\bin\aura.exe" (
    echo   ✓ Found aura.exe
) else (
    echo   ✗ Missing aura.exe
    set "VALID=0"
)

if exist "%INSTALL_PATH%\bin\aura-lsp.exe" (
    echo   ✓ Found aura-lsp.exe
) else (
    echo   ✗ Missing aura-lsp.exe
    set "VALID=0"
)

if exist "%INSTALL_PATH%\bin\aura-pkg.exe" (
    echo   ✓ Found aura-pkg.exe
) else (
    echo   ✗ Missing aura-pkg.exe
    set "VALID=0"
)

echo.
if "%VALID%"=="1" (
    echo ✅ Installation completed successfully!
    echo.
    echo Next steps:
    echo   1. Open a new Command Prompt and type: aura --version
    echo   2. Read %INSTALL_PATH%\README.md
    echo   3. Check examples in %INSTALL_PATH%\examples
    echo.
) else (
    echo ⚠️  Installation completed with issues
    echo Please check: %INSTALL_PATH%
)

echo Installation path: %INSTALL_PATH%
echo.
pause
