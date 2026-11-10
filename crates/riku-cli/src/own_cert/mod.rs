//! Service layer for an app's own (third party) TLS certificate: splitting
//! and checking a PEM bundle, reading names and expiry, and keeping the pair
//! and its marker in `nginx_root`. `riku certs` and `riku doctor` sit on top.

pub(crate) mod bundle;
pub(crate) mod info;
pub(crate) mod store;
#[cfg(test)]
pub(crate) mod test_pems;
