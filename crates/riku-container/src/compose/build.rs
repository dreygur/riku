//! The `build` key.

use super::paths::resolve_inside;
use super::scalar::{reject_unknown, string_map};
use super::{BuildDef, ServiceContext, Value};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// Parse `build`, either a context string or a mapping of `context`,
/// `dockerfile` and `args`. The returned context and Dockerfile are absolute
/// and inside the app directory; a relative Dockerfile is resolved against the
/// context, as compose does, and must stay inside the context because the
/// engine build refuses one outside it. An absolute context also means the
/// engine never reads the value as a URL.
pub(super) fn parse_build(ctx: &ServiceContext, value: &Value) -> Result<BuildDef> {
    let fields = match value {
        Value::String(context) => {
            return resolve_context(ctx, context).map(|context| BuildDef {
                context,
                dockerfile: None,
                args: BTreeMap::new(),
            });
        }
        Value::Object(fields) => fields,
        _ => bail!("build must be a string or a mapping"),
    };
    if let Err(e) = reject_unknown(fields, &["context", "dockerfile", "args"], "build") {
        return Err(e);
    }
    let context = match fields.get("context") {
        Some(Value::String(context)) => resolve_context(ctx, context),
        Some(_) => bail!("build context must be a string"),
        None => resolve_context(ctx, "."),
    };
    let context = match context {
        Ok(context) => context,
        Err(e) => return Err(e),
    };
    let dockerfile = match fields.get("dockerfile") {
        Some(Value::String(file)) => {
            match resolve_inside(ctx.app_path, &context, file, "build dockerfile") {
                Ok(path) if path.starts_with(&context) => Some(path),
                Ok(_) => bail!(
                    "build dockerfile {file:?} must be inside the build context {}",
                    context.display()
                ),
                Err(e) => return Err(e),
            }
        }
        Some(_) => bail!("build dockerfile must be a string"),
        None => None,
    };
    let args = match fields.get("args") {
        Some(args) => match string_map(args, "build arg") {
            Ok(args) => args,
            Err(e) => return Err(e),
        },
        None => BTreeMap::new(),
    };
    Ok(BuildDef {
        context,
        dockerfile,
        args,
    })
}

fn resolve_context(ctx: &ServiceContext, raw: &str) -> Result<std::path::PathBuf> {
    resolve_inside(ctx.app_path, ctx.compose_dir, raw, "build context")
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
