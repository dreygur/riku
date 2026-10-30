//! What a router is asked to route for one app.

use crate::backend::Backend;
use crate::domain::Domain;

/// How a site answers plain HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpsPolicy {
    /// Caddy's automatic HTTP to HTTPS redirect applies.
    Only,
    /// Plain HTTP is served too, through explicit `http://` addresses.
    AlsoPlainHttp,
}

/// An app's domains, their HTTPS policy and the backends that serve them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    pub domains: Vec<Domain>,
    pub https: HttpsPolicy,
    /// Every copy of the routed service. Routers balance across all of them.
    pub backends: Vec<Backend>,
}
