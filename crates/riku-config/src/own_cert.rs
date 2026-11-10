//! The marker that says an app serves a certificate its owner supplied.
//!
//! The pair itself lives in `nginx_root` as `<app>.key` and
//! `<app>.fullchain.crt`, the same files acme.sh and the fallback use. The
//! marker `<app>.own-cert` beside them tells riku not to issue, renew or
//! replace that pair, and tells the router to serve it whatever the https
//! policy is. This module only names the path and checks it exists.

use crate::RikuPaths;

/// The marker file's extension: `<nginx_root>/<app>.own-cert`.
pub const OWN_CERT_SUFFIX: &str = "own-cert";

/// Whether `app` has an own certificate marker in `paths.nginx_root`.
pub fn has_own_cert(paths: &RikuPaths, app: &str) -> bool {
    paths
        .nginx_root
        .join(format!("{app}.{OWN_CERT_SUFFIX}"))
        .exists()
}
