# Windows Packaging for Regent

This directory contains scripts and configurations for building Windows installers.

## Package Types

### MSI Installer (Recommended)

Full installer with:
- Start Menu shortcuts
- Automatic PATH setup
- Uninstaller
- Windows Installer integration

### Portable ZIP

Standalone executable with no installation required.

## Building

### Prerequisites

#### For MSI Installer

1. **Install WiX Toolset**
   ```powershell
   # Using Chocolatey
   choco install wixtoolset
   
   # Or download from https://wixtoolset.org/
   ```

2. **Install Visual Studio Build Tools**
   ```powershell
   # Required for Windows builds
   choco install visualstudio2022buildtools
   ```

3. **Install Rust with MSVC target**
   ```powershell
   rustup target add x86_64-pc-windows-msvc
   ```

#### For Portable ZIP

Only Rust with MSVC target is required.

### Build MSI Installer

```batch
cd packaging\windows
build-msi.bat
```

This creates `regent-0.1.1-x64.msi`

### Build Portable ZIP

```batch
cd packaging\windows
build-portable.bat
```

This creates `regent-0.1.1-windows-x64-portable.zip`

## Testing

### Test MSI Installation

```batch
REM Install with logging
msiexec /i regent-0.1.1-x64.msi /l*v install.log

REM Verify installation
regent --version
where regent

REM Uninstall
msiexec /x regent-0.1.1-x64.msi
```

### Test Portable Version

```batch
REM Extract ZIP
powershell Expand-Archive -Path regent-0.1.1-windows-x64-portable.zip -DestinationPath test-portable

REM Run directly
test-portable\regent.exe --version

REM Test with PATH
set PATH=%CD%\test-portable;%PATH%
regent --version
```

## Code Signing (Recommended for Release)

### Using SignTool

```batch
REM Sign MSI installer
signtool sign /f your-certificate.pfx /p password /t http://timestamp.digicert.com regent-0.1.1-x64.msi

REM Verify signature
signtool verify /pa regent-0.1.1-x64.msi
```

### Using Azure SignTool (for Azure Key Vault)

```batch
AzureSignTool sign ^
  -kvu https://yourvault.vault.azure.net ^
  -kvi %AZURE_CLIENT_ID% ^
  -kvt %AZURE_TENANT_ID% ^
  -kvs %AZURE_CLIENT_SECRET% ^
  -kvc YourCertName ^
  -tr http://timestamp.digicert.com ^
  -td sha256 ^
  regent-0.1.1-x64.msi
```

## Customization

### Change Product GUID

Edit `regent.wxs` and update the `UpgradeCode`:

```xml
<Product Id="*" UpgradeCode="YOUR-NEW-GUID-HERE">
```

Generate new GUID with PowerShell:
```powershell
[guid]::NewGuid()
```

### Add Custom Icon

1. Place your icon in `packaging/windows/regent.ico`
2. Icon should be 256x256 or smaller
3. Rebuild MSI

### Modify Installation Directory

Edit `regent.wxs`:
```xml
<Directory Id="INSTALLFOLDER" Name="YourCustomName">
```

## Distribution

### Via Chocolatey

Create `regent.nuspec`:

```xml
<?xml version="1.0"?>
<package>
  <metadata>
    <id>regent</id>
    <version>0.1.1</version>
    <title>Regent PDK</title>
    <authors>Felipe Quintella</authors>
    <description>High-performance Puppet Development Kit</description>
    <projectUrl>https://github.com/seu-usuario/regent</projectUrl>
    <tags>puppet pdk rust development</tags>
  </metadata>
  <files>
    <file src="regent-0.1.1-x64.msi" target="tools" />
  </files>
</package>
```

Build and publish:
```batch
choco pack
choco push regent.0.1.1.nupkg --source https://push.chocolatey.org/
```

### Via Scoop

Create manifest in a Scoop bucket:

```json
{
    "version": "0.1.1",
    "description": "High-performance Puppet Development Kit",
    "homepage": "https://github.com/seu-usuario/regent",
    "license": "AGPL-3.0",
    "url": "https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-windows-x64-portable.zip",
    "hash": "sha256:...",
    "bin": "regent.exe",
    "checkver": "github",
    "autoupdate": {
        "url": "https://github.com/seu-usuario/regent/releases/download/v$version/regent-$version-windows-x64-portable.zip"
    }
}
```

### Via WinGet

Create manifest:

```yaml
PackageIdentifier: YourPublisher.Regent
PackageVersion: 0.1.1
PackageLocale: en-US
Publisher: Felipe Quintella
PackageName: Regent
License: AGPL-3.0
ShortDescription: High-performance Puppet Development Kit
Installers:
  - Architecture: x64
    InstallerType: msi
    InstallerUrl: https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-x64.msi
    InstallerSha256: <sha256>
```

## Troubleshooting

### WiX build fails with "candle.exe not found"

Add WiX to PATH:
```batch
set PATH=%PATH%;C:\Program Files (x86)\WiX Toolset v3.11\bin
```

### MSI installation fails

Check install log:
```batch
type install.log | findstr /i "error"
```

### "regent.exe is not recognized"

After MSI installation, restart your terminal to refresh PATH.

## Files

- `regent.wxs` - WiX installer definition
- `build-msi.bat` - MSI build script
- `build-portable.bat` - Portable ZIP build script
- `regent.ico` - Application icon (to be created)
- `License.rtf` - License in RTF format for installer
- `README.md` - This file

## Resources

- [WiX Toolset Documentation](https://wixtoolset.org/documentation/)
- [Windows Installer Best Practices](https://docs.microsoft.com/en-us/windows/win32/msi/windows-installer-best-practices)
- [Code Signing for Windows](https://docs.microsoft.com/en-us/windows/win32/seccrypto/signtool)
