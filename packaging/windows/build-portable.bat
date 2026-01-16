@echo off
REM Create portable ZIP distribution for Regent

setlocal

set VERSION=0.1.1

echo Creating portable ZIP for Regent v%VERSION%...

REM Build the release binary
echo Building release binary...
cargo build --release --target x86_64-pc-windows-msvc

if not exist target\release\regent.exe (
    echo ERROR: regent.exe not found
    exit /b 1
)

REM Create distribution directory
set DIST_DIR=target\regent-%VERSION%-windows-portable
if exist %DIST_DIR% rmdir /s /q %DIST_DIR%
mkdir %DIST_DIR%

REM Copy files
echo Copying files...
copy target\release\regent.exe %DIST_DIR%\
copy LICENSE %DIST_DIR%\LICENSE.txt
copy README.md %DIST_DIR%\README.txt

REM Create README for portable version
echo Creating portable README...
(
echo Regent v%VERSION% - Portable Edition
echo ====================================
echo.
echo This is a portable version of Regent that does not require installation.
echo.
echo USAGE:
echo   1. Extract this ZIP to any directory
echo   2. Add the directory to your PATH, or
echo   3. Run regent.exe directly from this folder
echo.
echo MANUAL PATH SETUP:
echo   1. Press Win+X and select "System"
echo   2. Click "Advanced system settings"
echo   3. Click "Environment Variables"
echo   4. Under "User variables", select "Path" and click "Edit"
echo   5. Click "New" and add the full path to this directory
echo   6. Click "OK" on all windows
echo   7. Restart your command prompt
echo.
echo For more information, visit: https://github.com/seu-usuario/regent
) > %DIST_DIR%\PORTABLE-README.txt

REM Create ZIP
echo Creating ZIP archive...
powershell Compress-Archive -Path %DIST_DIR%\* -DestinationPath regent-%VERSION%-windows-x64-portable.zip -Force

if exist regent-%VERSION%-windows-x64-portable.zip (
    echo.
    echo SUCCESS: Portable ZIP created: regent-%VERSION%-windows-x64-portable.zip
    echo.
    echo Distribution includes:
    dir %DIST_DIR%
) else (
    echo ERROR: Failed to create ZIP
    exit /b 1
)

endlocal
