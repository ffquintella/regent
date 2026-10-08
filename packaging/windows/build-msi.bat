@echo off
REM Build Windows MSI installer for Regent (requires WiX Toolset v3 and Python).
setlocal
cd /d "%~dp0\..\.." || exit /b 1
set "VERSION=%~2"
if "%VERSION%"=="" set "VERSION=0.1.1"
set "ARCH=%~1"
if "%ARCH%"=="" set "ARCH=x64"
if not "%ARCH%"=="x64" (
    echo ERROR: Only x64 Windows packages are supported.
    exit /b 1
)
set "BINARY=target\x86_64-pc-windows-msvc\release\regent.exe"
set "GEM_CACHE=target\msi-bundled_gems"

echo Building Regent MSI installer v%VERSION% for %ARCH%...
python scripts\prepare-gem-cache.py
if errorlevel 1 exit /b 1
cargo build --release --target x86_64-pc-windows-msvc
if errorlevel 1 exit /b 1
if not exist "%BINARY%" (
    echo ERROR: regent.exe not found at %BINARY%
    exit /b 1
)
python scripts\prepare-gem-cache.py --stage "%GEM_CACHE%"
if errorlevel 1 exit /b 1

REM Convert the repository license to the RTF file consumed by WiX.
python -c "from pathlib import Path; b=chr(92); s=Path('LICENSE').read_text(); s=s.replace(b,b+b).replace('{',b+'{').replace('}',b+'}').replace(chr(10),b+'par '+chr(10)); Path('target/License.rtf').write_text('{'+b+'rtf1'+b+'ansi '+s+'}',encoding='ascii')"
if errorlevel 1 exit /b 1
REM Harvest every verified cache file, with a component per file.
heat dir "%GEM_CACHE%" -nologo -cg BundledGemComponents -dr BundledGemsFolder -srd -sreg -ag -var var.GemCacheDir -out target\bundled-gems.wxs
if errorlevel 1 exit /b 1
candle -nologo -arch x64 -dRegentVersion=%VERSION% -dRegentBinary="%BINARY%" -dGemCacheDir="%GEM_CACHE%" packaging\windows\regent.wxs target\bundled-gems.wxs -out target\
if errorlevel 1 exit /b 1
light -nologo target\regent.wixobj target\bundled-gems.wixobj -o regent-%VERSION%-%ARCH%.msi -ext WixUIExtension
if errorlevel 1 exit /b 1
if not exist regent-%VERSION%-%ARCH%.msi exit /b 1
echo SUCCESS: MSI package created: regent-%VERSION%-%ARCH%.msi
endlocal
