# Linux Packaging for Regent

This directory contains scripts for building Debian (.deb) and RPM packages.

## Debian Package (.deb)

### Build

```bash
# For AMD64
./packaging/linux/build-deb.sh 0.1.1 amd64

# For ARM64
./packaging/linux/build-deb.sh 0.1.1 arm64
```

### Install

```bash
sudo dpkg -i regent_0.1.1_amd64.deb
```

### Repository Distribution (Optional)

Create an APT repository:

```bash
# Install reprepro
sudo apt-get install reprepro

# Create repository structure
mkdir -p apt-repo/conf
cat > apt-repo/conf/distributions <<EOF
Origin: Regent
Label: Regent
Codename: stable
Architectures: amd64 arm64
Components: main
Description: Regent Puppet Development Kit
SignWith: YOUR_GPG_KEY_ID
EOF

# Add package
reprepro -b apt-repo includedeb stable regent_0.1.1_amd64.deb

# Serve repository (example with nginx)
sudo cp -r apt-repo /var/www/html/regent
```

Users can then install with:

```bash
echo "deb [trusted=yes] http://your-server.com/regent stable main" | sudo tee /etc/apt/sources.list.d/regent.list
sudo apt-get update
sudo apt-get install regent
```

## RPM Package (.rpm)

### Build

```bash
# For x86_64
./packaging/linux/build-rpm.sh 0.1.1 x86_64

# For aarch64
./packaging/linux/build-rpm.sh 0.1.1 aarch64
```

### Install

```bash
# Fedora/RHEL/CentOS
sudo rpm -ivh regent-0.1.1-1.x86_64.rpm

# Or with dnf
sudo dnf install ./regent-0.1.1-1.x86_64.rpm
```

### Repository Distribution (Optional)

Create a YUM repository:

```bash
# Create repository structure
mkdir -p yum-repo

# Copy RPMs
cp regent-0.1.1-1.*.rpm yum-repo/

# Create repository metadata
createrepo yum-repo/

# Serve repository (example with nginx)
sudo cp -r yum-repo /var/www/html/regent
```

Users can then install with:

```bash
# Create repo file
sudo tee /etc/yum.repos.d/regent.repo <<EOF
[regent]
name=Regent Repository
baseurl=http://your-server.com/regent
enabled=1
gpgcheck=0
EOF

sudo dnf install regent
```

## Requirements

### For Debian Packaging

```bash
sudo apt-get install dpkg-dev fakeroot
```

### For RPM Packaging

```bash
# Fedora/RHEL/CentOS
sudo dnf install rpm-build rpmdevtools

# Setup RPM build environment
rpmdev-setuptree
```

## Cross-Compilation

For building packages for different architectures:

```bash
# Install cross-compilation tools
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu

# Use cross for easier cross-compilation
cargo install cross

# Build for different target
cross build --release --target aarch64-unknown-linux-gnu
```

## Package Testing

### Test in Docker

```bash
# Debian
docker run -it --rm -v $(pwd):/work debian:12 bash
cd /work
dpkg -i regent_0.1.1_amd64.deb
regent --version

# Fedora
docker run -it --rm -v $(pwd):/work fedora:39 bash
cd /work
dnf install -y ./regent-0.1.1-1.x86_64.rpm
regent --version
```

## Release Checklist

- [ ] Build Debian packages for amd64 and arm64
- [ ] Build RPM packages for x86_64 and aarch64
- [ ] Test installation on clean systems
- [ ] Sign packages with GPG
- [ ] Upload to repository
- [ ] Update installation documentation
- [ ] Test repository installation

## Troubleshooting

### "regent: command not found" after installation

Check if `/usr/bin` is in your PATH:
```bash
echo $PATH
```

### Permission denied

Ensure the binary is executable:
```bash
ls -l /usr/bin/regent
```

### Dependencies not satisfied

Check package dependencies:
```bash
# Debian
dpkg -I regent_0.1.1_amd64.deb

# RPM
rpm -qpR regent-0.1.1-1.x86_64.rpm
```
