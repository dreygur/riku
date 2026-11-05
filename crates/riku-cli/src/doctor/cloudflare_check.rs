//! `riku doctor` check for `/etc/riku/cloudflare.conf`, the address list a
//! site with the Cloudflare ACL includes. Root init writes it and the router
//! helper never fetches it, so it ages until `sudo riku init` runs again.

use std::path::Path;
use std::time::{Duration, SystemTime};

use super::Check;

const NAME: &str = "cloudflare ranges";
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);
const REFRESH_HINT: &str = "sudo riku init";

fn age_of(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = path.metadata().and_then(|meta| meta.modified()).ok()?;
    Some(now.duration_since(modified).unwrap_or_default())
}

/// Warn when `file` is missing or older than 30 days. Only apps that set
/// `RIKU_NGINX_CLOUDFLARE_ACL` use it, so this never fails.
pub fn cloudflare_ranges(file: &Path, now: SystemTime) -> Check {
    match age_of(file, now) {
        None => Check::warn(
            NAME,
            format!(
                "{} is missing, apps with RIKU_NGINX_CLOUDFLARE_ACL are refused. Fetch it with: {REFRESH_HINT}",
                file.display()
            ),
        ),
        Some(age) if age > MAX_AGE => Check::warn(
            NAME,
            format!(
                "{} is {} days old. Refresh it with: {REFRESH_HINT}",
                file.display(),
                age.as_secs() / 86400
            ),
        ),
        Some(_) => Check::ok(NAME, format!("{} is current", file.display())),
    }
}

#[cfg(test)]
#[path = "cloudflare_check_tests.rs"]
mod tests;
