//! Build script. Codegen `SkillHost` enum, `KNOWN_HOSTS` const,
//! `resolve_host` / `host_envelope_str` / `config_dir_env` / `base_dir_env` /
//! `legacy_destination` functions, `CONFIG_DIR_VARS`, `BASE_DIR_VARS`, and the
//! `bird skill --help` environment lines from `src/skill_install/skill.json`.
//!
//! The manifest is the single source of truth — updating the JSON regenerates
//! the Rust map on the next `cargo build` (via `cargo:rerun-if-changed`).
//! Mirrors the pattern in `xurl-rs/crates/xurl-cli/build.rs` and
//! `agentnative-cli/build.rs`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// One install host: JSON key, enum variant, clone URL, `~` destination.
type Host = (String, String, String, String);

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    emit_skill_hosts(&manifest_dir);
}

/// Emit `$OUT_DIR/generated_hosts.rs` and `$OUT_DIR/skill_env_help.txt` from
/// `src/skill_install/skill.json`.
///
/// Reads the manifest's `install` map and emits, for every `<host>` key:
///
/// - a `SkillHost` enum variant (PascalCase of the snake_case key) with
///   `clap::ValueEnum` derive + `#[value(rename_all = "snake_case")]` so
///   surface names round-trip back to the JSON key verbatim;
/// - an entry in `KNOWN_HOSTS: &[&str]`;
/// - a match arm in `resolve_host(SkillHost) -> (&'static str, &'static str)`
///   returning the `(url, dest_template)` parsed from the host's install
///   command;
/// - a match arm in `host_envelope_str(SkillHost) -> &'static str` returning
///   the canonical JSON-key surface name;
/// - a match arm in `config_dir_env(SkillHost)` from the host's
///   `config_dir_env` entry, with its variable in `CONFIG_DIR_VARS` and a help
///   line in `skill_env_help.txt`;
/// - a match arm in `base_dir_env(SkillHost)` from the host's optional
///   `base_dir_env` entry, with its variable in `BASE_DIR_VARS` and a help
///   line of its own;
/// - a match arm in `legacy_destination(SkillHost)` from the host's optional
///   `legacy_destinations` entry.
///
/// Each install command MUST have the canonical shape
/// `git clone --depth 1 <url> <dest>` — six whitespace-separated tokens.
/// Anything else panics the build with the offending host and command.
fn emit_skill_hosts(manifest_dir: &Path) {
    let skill_json_path = manifest_dir.join("src/skill_install/skill.json");
    println!("cargo:rerun-if-changed=src/skill_install/skill.json");

    let content = fs::read_to_string(&skill_json_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", skill_json_path.display()));
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("parse {}: {e}", skill_json_path.display()));

    let install = manifest
        .get("install")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| {
            panic!(
                "{}: \"install\" must be an object (host -> command map)",
                skill_json_path.display()
            )
        });

    if install.is_empty() {
        panic!(
            "{}: install map is empty — at least one host required",
            skill_json_path.display()
        );
    }

    // Sorted by JSON key so the generated source has stable byte output.
    let mut hosts: Vec<Host> = Vec::with_capacity(install.len());
    for (key, cmd_value) in install {
        let cmd = cmd_value.as_str().unwrap_or_else(|| {
            panic!(
                "{}: install.{key:?} must be a string",
                skill_json_path.display()
            )
        });
        let tokens: Vec<&str> = cmd.split_whitespace().collect();
        if tokens.len() != 6
            || tokens[0] != "git"
            || tokens[1] != "clone"
            || tokens[2] != "--depth"
            || tokens[3] != "1"
        {
            panic!(
                "{}: install.{key:?} must match `git clone --depth 1 <url> <dest>` (got {} tokens: {cmd:?})",
                skill_json_path.display(),
                tokens.len(),
            );
        }
        let url = tokens[4].to_string();
        let dest = tokens[5].to_string();
        if dest.ends_with(".git") {
            panic!(
                "{}: install.{key:?} dest {dest:?} ends in `.git` — host commands must terminate with an explicit destination, not the bare repo name",
                skill_json_path.display()
            );
        }
        let variant = pascal_case(key).unwrap_or_else(|e| {
            panic!(
                "{}: install.{key:?} is not a valid Rust identifier: {e}",
                skill_json_path.display()
            )
        });
        hosts.push((key.clone(), variant, url, dest));
    }
    hosts.sort_by(|a, b| a.0.cmp(&b.0));

    let mut seen_vars: Vec<String> = Vec::new();
    let config_dirs = parse_config_dir_env(&manifest, &hosts, &mut seen_vars, &skill_json_path);
    let base_dirs = parse_base_dir_env(&manifest, &hosts, &mut seen_vars, &skill_json_path);
    let legacy = parse_legacy_destinations(&manifest, &hosts, &skill_json_path);

    let mut src = String::new();
    src.push_str(
        "// @generated by build.rs from src/skill_install/skill.json. Do not edit by hand.\n",
    );
    src.push_str(
        "// Add or remove hosts via the JSON file and `cargo build` regenerates this file.\n\n",
    );

    src.push_str("/// Hosts the binary knows how to install into. Surface names match\n");
    src.push_str("/// `src/skill_install/skill.json` keys verbatim via\n");
    src.push_str("/// `rename_all = \"snake_case\"`.\n");
    src.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, ::clap::ValueEnum)]\n");
    src.push_str("#[value(rename_all = \"snake_case\")]\n");
    src.push_str("pub enum SkillHost {\n");
    for (_, variant, _, _) in &hosts {
        src.push_str(&format!("    {variant},\n"));
    }
    src.push_str("}\n\n");

    src.push_str(
        "/// Host names accepted by `bird skill install <host>`, in JSON-key sort order.\n",
    );
    src.push_str("/// Stays in lockstep with [`SkillHost`] because both are generated from the\n");
    src.push_str("/// same source.\n");
    src.push_str("pub const KNOWN_HOSTS: &[&str] = &[\n");
    for (key, _, _, _) in &hosts {
        src.push_str(&format!("    {key:?},\n"));
    }
    src.push_str("];\n\n");

    src.push_str("/// Resolve a host enum to its `(url, dest_template)` pair, parsed at build\n");
    src.push_str("/// time from the install command in `src/skill_install/skill.json`. Pure\n");
    src.push_str("/// function — no I/O, no side effects.\n");
    src.push_str("pub fn resolve_host(host: SkillHost) -> (&'static str, &'static str) {\n");
    src.push_str("    match host {\n");
    for (_, variant, url, dest) in &hosts {
        src.push_str(&format!(
            "        SkillHost::{variant} => ({url:?}, {dest:?}),\n"
        ));
    }
    src.push_str("    }\n");
    src.push_str("}\n\n");

    src.push_str("/// JSON-key string for the envelope's `host` field. Generated alongside the\n");
    src.push_str("/// enum so the surface stays in lockstep with the JSON contract.\n");
    src.push_str("pub fn host_envelope_str(host: SkillHost) -> &'static str {\n");
    src.push_str("    match host {\n");
    for (key, variant, _, _) in &hosts {
        src.push_str(&format!("        SkillHost::{variant} => {key:?},\n"));
    }
    src.push_str("    }\n");
    src.push_str("}\n\n");

    emit_var_fn(
        &mut src,
        &hosts,
        &config_dirs,
        "config_dir_env",
        "CONFIG_DIR_VARS",
        &[
            "The variable `host` reads to relocate the directory its skills live",
            "under, and the destination prefix that variable's value stands in for;",
            "`None` for a host that documents no such variable.",
        ],
        "Every host config-dir variable, in the JSON-key order of its host.",
    );
    emit_var_fn(
        &mut src,
        &hosts,
        &base_dirs,
        "base_dir_env",
        "BASE_DIR_VARS",
        &[
            "The standard base-directory variable `host` follows for a directory",
            "above its skills, and the destination prefix that variable's value",
            "stands in for. It applies only while `BIRD_SKILL_HOME` is unset, since",
            "that variable replaces the whole home; `None` for a host that follows",
            "no such variable.",
        ],
        "Every base-directory variable, in the JSON-key order of its host.",
    );

    src.push_str("/// A `~`-prefixed location other than the destination where `host` still\n");
    src.push_str("/// reads skills, so `skill update` removes a copy of the bundle found there;\n");
    src.push_str("/// `None` for a host with no such location.\n");
    src.push_str("pub fn legacy_destination(host: SkillHost) -> Option<&'static str> {\n");
    src.push_str("    match host {\n");
    for ((_, variant, _, _), entry) in hosts.iter().zip(&legacy) {
        match entry {
            Some(path) => src.push_str(&format!(
                "        SkillHost::{variant} => Some({path:?}),\n"
            )),
            None => src.push_str(&format!("        SkillHost::{variant} => None,\n")),
        }
    }
    src.push_str("    }\n");
    src.push_str("}\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_path = out_dir.join("generated_hosts.rs");
    fs::write(&out_path, src)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", out_path.display()));
    let help_path = out_dir.join("skill_env_help.txt");
    fs::write(&help_path, skill_env_help(&hosts, &config_dirs, &base_dirs))
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", help_path.display()));
}

/// One `bird skill --help` environment line per variable, so the help cannot
/// fall behind the manifest.
fn skill_env_help(
    hosts: &[Host],
    config_dirs: &[Option<(String, String)>],
    base_dirs: &[Option<(String, String)>],
) -> String {
    let mut help = String::from("Environment:\n");
    help.push_str(&format!(
        "  {:<22} Stands in for HOME in every skill destination\n",
        "BIRD_SKILL_HOME"
    ));
    for ((key, _, _, _), entry) in hosts.iter().zip(config_dirs) {
        if let Some((var, replaces)) = entry {
            help.push_str(&format!(
                "  {var:<22} Stands in for {replaces} in the {key} skill destination; wins over BIRD_SKILL_HOME\n"
            ));
        }
    }
    for ((key, _, _, _), entry) in hosts.iter().zip(base_dirs) {
        if let Some((var, replaces)) = entry {
            help.push_str(&format!(
                "  {var:<22} Stands in for {replaces} in the {key} skill destination when BIRD_SKILL_HOME is unset\n"
            ));
        }
    }
    help
}

/// Emit `pub fn <name>(SkillHost) -> Option<(&str, &str)>` over `entries`
/// (one per host, in `hosts` order) and `pub const <consts>` listing each
/// variable that is present.
fn emit_var_fn(
    src: &mut String,
    hosts: &[Host],
    entries: &[Option<(String, String)>],
    name: &str,
    consts: &str,
    fn_doc: &[&str],
    const_doc: &str,
) {
    for line in fn_doc {
        src.push_str(&format!("/// {line}\n"));
    }
    src.push_str(&format!(
        "pub fn {name}(host: SkillHost) -> Option<(&'static str, &'static str)> {{\n"
    ));
    src.push_str("    match host {\n");
    for ((_, variant, _, _), entry) in hosts.iter().zip(entries) {
        match entry {
            Some((var, replaces)) => src.push_str(&format!(
                "        SkillHost::{variant} => Some(({var:?}, {replaces:?})),\n"
            )),
            None => src.push_str(&format!("        SkillHost::{variant} => None,\n")),
        }
    }
    src.push_str("    }\n");
    src.push_str("}\n\n");

    src.push_str(&format!("/// {const_doc}\n"));
    src.push_str(&format!("pub const {consts}: &[&str] = &[\n"));
    for (var, _) in entries.iter().flatten() {
        src.push_str(&format!("    {var:?},\n"));
    }
    src.push_str("];\n\n");
}

/// The manifest's `section` map, with every key checked against `hosts`.
/// A missing section is an error only when `required`.
fn host_map<'a>(
    manifest: &'a serde_json::Value,
    section: &str,
    required: bool,
    hosts: &[Host],
    path: &Path,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    let Some(value) = manifest.get(section) else {
        if required {
            panic!(
                "{}: \"{section}\" must be an object (host -> entry map)",
                path.display()
            );
        }
        return None;
    };
    let map = value.as_object().unwrap_or_else(|| {
        panic!(
            "{}: \"{section}\" must be an object (host -> entry map)",
            path.display()
        )
    });
    for key in map.keys() {
        if !hosts.iter().any(|(host, _, _, _)| host == key) {
            panic!(
                "{}: {section}.{key:?} names no host in the install map",
                path.display()
            );
        }
    }
    Some(map)
}

/// Vet one entry's string `var` and `replaces`, returning the
/// `(var, replaces)` pair. `var` must be an upper-case name other than `HOME`
/// and `BIRD_SKILL_HOME` that no earlier entry in either section claimed;
/// `replaces` must be a `~/` directory prefix of `dest`.
fn parse_var(
    section: &str,
    key: &str,
    entry: &serde_json::Map<String, serde_json::Value>,
    dest: &str,
    seen: &mut Vec<String>,
    path: &Path,
) -> (String, String) {
    let var = entry.get("var").and_then(|v| v.as_str()).unwrap_or("");
    let valid = var.starts_with(|c: char| c.is_ascii_uppercase())
        && var
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !valid || var == "HOME" || var == "BIRD_SKILL_HOME" || seen.iter().any(|s| s == var) {
        panic!(
            "{}: {section}.{key:?}.var {var:?} must be a distinct upper-case variable name other than HOME and BIRD_SKILL_HOME",
            path.display()
        );
    }
    seen.push(var.to_string());
    let replaces = entry.get("replaces").and_then(|v| v.as_str()).unwrap_or("");
    if !replaces.starts_with("~/")
        || replaces.ends_with('/')
        || !dest.starts_with(&format!("{replaces}/"))
    {
        panic!(
            "{}: {section}.{key:?}.replaces {replaces:?} must be a ~/ directory prefix of the destination {dest:?}",
            path.display()
        );
    }
    (var.to_string(), replaces.to_string())
}

/// Require an https `source` on `section.key`, the evidence for its claim.
fn require_source(
    section: &str,
    key: &str,
    entry: &serde_json::Map<String, serde_json::Value>,
    path: &Path,
) {
    let source = entry.get("source").and_then(|v| v.as_str()).unwrap_or("");
    if !source.starts_with("https://") {
        panic!(
            "{}: {section}.{key:?}.source must be an https URL",
            path.display()
        );
    }
}

/// Parse and vet the manifest's `config_dir_env` map, returning each host's
/// `(var, replaces)` in the order of `hosts`.
///
/// Every install host must carry an entry, so adding a host forces a decision
/// about its variable. An entry names a `source` for the claim and either a
/// `var` with the `replaces` prefix it stands in for, or `var: null` with a
/// `note` saying why the host has none.
fn parse_config_dir_env(
    manifest: &serde_json::Value,
    hosts: &[Host],
    seen: &mut Vec<String>,
    path: &Path,
) -> Vec<Option<(String, String)>> {
    let section = "config_dir_env";
    let map = host_map(manifest, section, true, hosts, path).expect("required section");
    hosts
        .iter()
        .map(|(key, _, _, dest)| {
            let entry = map.get(key).and_then(|v| v.as_object()).unwrap_or_else(|| {
                panic!(
                    "{}: {section}.{key:?} is missing; every install host needs an entry",
                    path.display()
                )
            });
            require_source(section, key, entry, path);
            match entry.get("var") {
                Some(serde_json::Value::Null) => {
                    let note = entry.get("note").and_then(|v| v.as_str()).unwrap_or("");
                    if note.trim().is_empty() || entry.contains_key("replaces") {
                        panic!(
                            "{}: {section}.{key:?} has var null, so it needs a note and no replaces",
                            path.display()
                        );
                    }
                    None
                }
                Some(serde_json::Value::String(_)) => {
                    Some(parse_var(section, key, entry, dest, seen, path))
                }
                _ => panic!(
                    "{}: {section}.{key:?}.var must be a string or null",
                    path.display()
                ),
            }
        })
        .collect()
}

/// Parse and vet the manifest's optional `base_dir_env` map, returning each
/// host's `(var, replaces)` in the order of `hosts`. Only a host that follows
/// a standard base-directory variable carries an entry, with a `source`, the
/// `var`, and the `replaces` prefix it stands in for.
fn parse_base_dir_env(
    manifest: &serde_json::Value,
    hosts: &[Host],
    seen: &mut Vec<String>,
    path: &Path,
) -> Vec<Option<(String, String)>> {
    let section = "base_dir_env";
    let Some(map) = host_map(manifest, section, false, hosts, path) else {
        return vec![None; hosts.len()];
    };
    hosts
        .iter()
        .map(|(key, _, _, dest)| {
            let entry = map.get(key)?.as_object().unwrap_or_else(|| {
                panic!("{}: {section}.{key:?} must be an object", path.display())
            });
            require_source(section, key, entry, path);
            Some(parse_var(section, key, entry, dest, seen, path))
        })
        .collect()
}

/// Parse and vet the manifest's optional `legacy_destinations` map, returning
/// each host's `path` in the order of `hosts`. An entry names a `~/` path other
/// than the host's destination, the `source` showing the host still reads it,
/// and a `note`.
fn parse_legacy_destinations(
    manifest: &serde_json::Value,
    hosts: &[Host],
    path: &Path,
) -> Vec<Option<String>> {
    let section = "legacy_destinations";
    let Some(map) = host_map(manifest, section, false, hosts, path) else {
        return vec![None; hosts.len()];
    };
    hosts
        .iter()
        .map(|(key, _, _, dest)| {
            let entry = map.get(key)?.as_object().unwrap_or_else(|| {
                panic!("{}: {section}.{key:?} must be an object", path.display())
            });
            require_source(section, key, entry, path);
            let legacy = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let note = entry.get("note").and_then(|v| v.as_str()).unwrap_or("");
            if !legacy.starts_with("~/") || legacy.ends_with('/') || legacy == dest {
                panic!(
                    "{}: {section}.{key:?}.path {legacy:?} must be a ~/ path other than the destination",
                    path.display()
                );
            }
            if note.trim().is_empty() {
                panic!("{}: {section}.{key:?} needs a note", path.display());
            }
            Some(legacy.to_string())
        })
        .collect()
}

/// Convert a snake_case ASCII identifier to PascalCase. Rejects empty
/// strings, leading digits, and any character outside `[a-z0-9_]` so the
/// emitted variant is always a valid Rust identifier.
fn pascal_case(snake: &str) -> Result<String, String> {
    if snake.is_empty() {
        return Err("empty identifier".into());
    }
    if snake.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(format!("{snake:?} starts with a digit"));
    }
    if !snake
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "{snake:?} contains non-snake_case ASCII characters"
        ));
    }
    let mut out = String::with_capacity(snake.len());
    for word in snake.split('_') {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    Ok(out)
}
