@echo off
REM Build Windows MSI installer for Regent
REM Requires: WiX Toolset (https://wixtoolset.org/)

setlocal

set VERSION=0.1.1
set ARCH=%1

if "%ARCH%"=="" set ARCH=x64

echo Building Regent MSI installer v%VERSION% for %ARCH%...

REM Build the release binary
echo Building release binary...
cargo build --release --target x86_64-pc-windows-msvc

if not exist target\release\regent.exe (
    echo ERROR: regent.exe not found in target\release\
    exit /b 1
)

REM Build MSI with WiX
echo Building MSI package...
candle packaging\windows\regent.wxs -o target\regent.wixobj
light target\regent.wixobj -o regent-%VERSION%-%ARCH%.msi -ext WixUIExtension

if exist regent-%VERSION%-%ARCH%.msi (
    echo.
    echo SUCCESS: MSI package created: regent-%VERSION%-%ARCH%.msi
    echo.
    echo Test installation with:
    echo   msiexec /i regent-%VERSION%-%ARCH%.msi /l*v install.log
    echo.
    echo Uninstall with:
    echo   msiexec /x regent-%VERSION%-%ARCH%.msi
) else (
    echo ERROR: Failed to create MSI package
    exit /b 1
)

endlocal
