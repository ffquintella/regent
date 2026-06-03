use anyhow::Result;
use md5;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub struct ChecksumGenerator {
    pub file_path: PathBuf,
}

impl ChecksumGenerator {
    pub fn new(file_path: &Path) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
        }
    }

    /// Generate SHA256 checksum
    pub fn sha256(&self) -> Result<String> {
        let file = File::open(&self.file_path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let digest = hasher.finalize();
        Ok(digest.iter().map(|b| format!("{:02x}", b)).collect())
    }

    /// Generate MD5 checksum
    pub fn md5(&self) -> Result<String> {
        let file = File::open(&self.file_path)?;
        let mut reader = BufReader::new(file);
        let mut context = md5::Context::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            context.consume(&buffer[..count]);
        }

        Ok(format!("{:x}", context.finalize()))
    }

    /// Generate both checksums
    pub fn generate_all(&self) -> Result<ChecksumSet> {
        Ok(ChecksumSet {
            sha256: self.sha256()?,
            md5: self.md5()?,
        })
    }
}

pub struct ChecksumSet {
    pub sha256: String,
    pub md5: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_sha256_generation() {
        let content = b"test content";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();
        let generator = ChecksumGenerator::new(&path);
        let sha256 = generator.sha256().unwrap();

        // SHA256 should be 64 hex characters
        assert_eq!(sha256.len(), 64);
        assert!(!sha256.is_empty());
    }

    #[test]
    fn test_md5_generation() {
        let content = b"test content";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();
        let generator = ChecksumGenerator::new(&path);
        let md5 = generator.md5().unwrap();

        // MD5 should be 32 hex characters
        assert_eq!(md5.len(), 32);
        assert!(!md5.is_empty());
    }

    #[test]
    fn test_generate_all() {
        let content = b"test content";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();
        let generator = ChecksumGenerator::new(&path);
        let checksums = generator.generate_all().unwrap();

        assert_eq!(checksums.sha256.len(), 64);
        assert_eq!(checksums.md5.len(), 32);
    }

    #[test]
    fn test_checksum_consistency() {
        let content = b"test content";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();
        let generator = ChecksumGenerator::new(&path);

        let sha256_1 = generator.sha256().unwrap();
        let sha256_2 = generator.sha256().unwrap();

        assert_eq!(sha256_1, sha256_2);
    }
}
