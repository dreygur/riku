//! Validated value types shared by the engine driver and the compose parser.
//!
//! # Security model
//!
//! Names and signals originate in a compose file, which is untrusted. Each
//! constructor here rejects anything an engine could misread as an option, a
//! path or a second argument, so a value of these types is safe to pass as one
//! argv element.

mod lifecycle;
mod mounts;
mod names;
mod ports;

pub use lifecycle::{HealthCheck, HealthTest, RestartPolicy, StopSignal, DEFAULT_STOP_GRACE};
pub use mounts::{Mount, MountSource};
pub use names::{check_app_name, labels, ContainerName, CopyIndex, NetworkName, ServiceName};
pub(crate) use names::{check_engine_name, is_engine_safe};
pub use ports::{HostIp, PortBinding, Protocol};
