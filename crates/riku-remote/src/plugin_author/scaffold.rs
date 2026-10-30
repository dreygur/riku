//! `plugins scaffold`: writes a starter bundle directory.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use riku_util::{display, validate_app_name};

use super::skeleton::SeamType;
use super::Program;

const ENTRY_MODE: u32 = 0o755;

fn write_bundle(
    bundle: &Path,
    name: &str,
    plugin_type: &str,
    seam: &SeamType,
    program: Program,
) -> Result<()> {
    if let Err(e) = fs::create_dir_all(bundle.join("bin")) {
        return Err(e.into());
    }
    if let Err(e) = fs::write(bundle.join("riku-plugin.toml"), seam.manifest(name)) {
        return Err(e.into());
    }
    let entry = bundle.join("bin").join(name);
    if let Err(e) = fs::write(&entry, seam.entry_script(name)) {
        return Err(e.into());
    }
    if let Err(e) = fs::set_permissions(&entry, fs::Permissions::from_mode(ENTRY_MODE)) {
        return Err(e.into());
    }
    let readme = format!(
        "# {name}\n\nA Riku {plugin_type} plugin. Edit `bin/{name}`, then:\n\n    {} plugins install ./{name}\n",
        program.name()
    );
    if let Err(e) = fs::write(bundle.join("README.md"), readme) {
        return Err(e.into());
    }
    Ok(())
}

/// `plugins scaffold <name> --type <type>`. `dir` is the directory the
/// bundle is created in, the current directory when `None`.
pub fn cmd_plugins_scaffold(
    program: Program,
    name: &str,
    plugin_type: &str,
    dir: Option<&str>,
) -> Result<()> {
    let name = match validate_app_name(name) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let seam = match SeamType::parse(plugin_type) {
        Ok(seam) => seam,
        Err(e) => return Err(e),
    };

    let bundle = dir.map(PathBuf::from).unwrap_or_default().join(&name);
    if bundle.exists() {
        bail!(
            "'{}' already exists, choose another name or path",
            bundle.display()
        );
    }
    if let Err(e) = write_bundle(&bundle, &name, plugin_type, &seam, program) {
        return Err(e);
    }

    let program = program.name();
    display::success(&format!(
        "Scaffolded {plugin_type} plugin in ./{}",
        bundle.display()
    ));
    display::note(&format!(
        "Edit bin/{name}, then: {program} plugins install ./{}",
        bundle.display()
    ));
    display::note(&format!(
        "Sign it for distribution: {program} plugins keygen && {program} plugins sign ./<dir> --key <file>"
    ));
    Ok(())
}
