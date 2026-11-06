//! Published port bindings.

/// Host interface a published port listens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostIp {
    /// 127.0.0.1 only. The default for every published port.
    Loopback,
    /// All interfaces. Only for services labelled `riku.public: "true"`.
    All,
}

/// Transport protocol of a published port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

/// One published port of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortBinding {
    pub host_ip: HostIp,
    /// `None` lets the engine pick a free host port.
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub protocol: Protocol,
}
