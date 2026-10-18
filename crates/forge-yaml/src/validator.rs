use forge_core::error::{ForgeError, Result};

pub const SUPPORTED_SCHEMA_VERSION: &str = "1.0";

/// Validates that the schema version declared in a YAML document is supported.
pub fn validate_schema_version(version: &str) -> Result<()> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.is_empty() {
        return Err(ForgeError::Validation(format!(
            "Invalid schema version string: '{version}'"
        )));
    }
    let major = parts[0];
    if major != "1" {
        return Err(ForgeError::Validation(format!(
            "Unsupported schema major version: '{version}' (expected 1.x)"
        )));
    }
    Ok(())
}

/// Validates that the entity kind matches expected kind.
pub fn validate_entity_kind(found: &str, expected: &str) -> Result<()> {
    if found != expected {
        return Err(ForgeError::Validation(format!(
            "Mismatched entity kind: expected '{expected}', found '{found}'"
        )));
    }
    Ok(())
}
