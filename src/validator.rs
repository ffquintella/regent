use std::path::Path;

pub struct Validator;

impl Validator {
    pub fn validate(path: &Path) -> anyhow::Result<()> {
        // Check for required module structure
        if !path.join("metadata.json").exists() {
            return Err(anyhow::anyhow!("Missing metadata.json"));
        }

        if !path.join("manifests").exists() {
            return Err(anyhow::anyhow!("Missing manifests directory"));
        }

        Ok(())
    }

    pub fn validate_metadata(path: &Path) -> anyhow::Result<()> {
        let metadata = std::fs::read_to_string(path.join("metadata.json"))?;
        let _: serde_json::Value = serde_json::from_str(&metadata)?;

        Ok(())
    }
}
