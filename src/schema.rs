//! OpenAPI schema: path template resolution with param substitution.

use std::collections::HashMap;

/// Validate that a path parameter value contains only safe characters.
fn validate_param_value(
    name: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if value.is_empty() {
        return Err(format!("path parameter '{}' must not be empty", name).into());
    }
    if !value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(format!(
            "path parameter '{}' contains invalid characters (only alphanumeric, underscore, hyphen, dot allowed): {}",
            name, value
        ).into());
    }
    Ok(())
}

/// Resolve path template into concrete path by substituting {param} with values.
/// Values come from params map (CLI -p), then env X_API_<PARAM_NAME> (uppercase, - → _).
pub fn resolve_path(
    path_template: &str,
    params: &HashMap<String, String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut out = path_template.to_string();
    let mut i = 0;
    while i < out.len() {
        if let Some(start) = out[i..].find('{') {
            let start = i + start;
            if let Some(end) = out[start..].find('}') {
                let end = start + end + 1;
                let name = &out[start + 1..end - 1];
                let value = params.get(name).cloned().or_else(|| {
                    let env_key = format!("X_API_{}", name.to_uppercase().replace('-', "_"));
                    std::env::var(&env_key).ok()
                });
                let value = value.ok_or_else(|| format!("missing path parameter: {}", name))?;
                validate_param_value(name, &value)?;
                out.replace_range(start..end, &value);
                i = start + value.len();
                continue;
            }
        }
        break;
    }
    Ok(out)
}
