//! OpenAPI schema: load bundled spec, extract path templates and path parameter names.

use std::collections::HashMap;
use std::path::Path;

/// Path templates and for each path the set of path parameter names.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct SchemaPaths {
    /// Path template -> list of path parameter names (e.g. "/2/users/{id}" -> ["id"]).
    pub path_params: HashMap<String, Vec<String>>,
}

/// Load schema from path (e.g. openapi/x-api-openapi.json) and extract paths + path param names.
#[allow(dead_code)]
pub fn load_schema_paths(path: &Path) -> Result<SchemaPaths, Box<dyn std::error::Error + Send + Sync>> {
    let s = std::fs::read_to_string(path)?;
    let spec: serde_json::Value = serde_json::from_str(&s)?;
    let paths = spec.get("paths").and_then(|p| p.as_object()).ok_or("no paths in spec")?;
    let mut path_params = HashMap::new();
    for (path_template, path_item) in paths {
        let param_names = path_params_for_path(path_item);
        path_params.insert(path_template.clone(), param_names);
    }
    Ok(SchemaPaths { path_params })
}

/// Extract path parameter names from a path item (all methods share the same path params in OpenAPI).
#[allow(dead_code)]
fn path_params_for_path(path_item: &serde_json::Value) -> Vec<String> {
    let mut names = Vec::new();
    let path_item = match path_item.as_object() {
        Some(o) => o,
        None => return names,
    };
    for (_method, op) in path_item {
        let op = match op.as_object() {
            Some(o) => o,
            None => continue,
        };
        for param in op.get("parameters").and_then(|p| p.as_array()).iter().flat_map(|a| a.iter()) {
            let param = match param.as_object() {
                Some(o) => o,
                None => continue,
            };
            if param.get("in").and_then(|v| v.as_str()) != Some("path") {
                continue;
            }
            if let Some(name) = param.get("name").and_then(|n| n.as_str()) {
                if !names.contains(&name.to_string()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
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
                let value = params
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        let env_key = format!("X_API_{}", name.to_uppercase().replace('-', "_"));
                        std::env::var(&env_key).ok()
                    });
                let value = value.ok_or_else(|| format!("missing path parameter: {}", name))?;
                out.replace_range(start..end, &value);
                i = start + value.len();
                continue;
            }
        }
        break;
    }
    Ok(out)
}
