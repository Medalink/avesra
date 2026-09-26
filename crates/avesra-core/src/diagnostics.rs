//! Fixed read catalog and bounded observations; these values are not authority.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HOST: Uuid = Uuid::from_u128(0x9a42c2ec_e532_42f5_9f41_3cbb07160201);
pub const VPN: Uuid = Uuid::from_u128(0x9a42c2ec_e532_42f5_9f41_3cbb07160202);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Catalog {
    HostResources,
    CiscoVpnStatus,
}
impl Catalog {
    pub fn name(self) -> &'static str {
        match self {
            Self::HostResources => "computer performance",
            Self::CiscoVpnStatus => "vpn status",
        }
    }
    pub fn id(self) -> Uuid {
        match self {
            Self::HostResources => HOST,
            Self::CiscoVpnStatus => VPN,
        }
    }
    pub fn from_id(id: Uuid) -> Result<Self, ErrorCode> {
        if id == HOST {
            Ok(Self::HostResources)
        } else if id == VPN {
            Ok(Self::CiscoVpnStatus)
        } else {
            Err(ErrorCode::Unsupported)
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    CounterUnavailable,
    CounterReset,
    Changed,
    Unsupported,
    Missing,
    Expired,
    Cancelled,
    OutputLimit,
    CommandFailed,
    UnrecognizedOutput,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reading<T> {
    Available { value: T },
    Unavailable { reason: Unavailable },
}
impl<T> Default for Reading<T> {
    fn default() -> Self {
        Self::Unavailable {
            reason: Unavailable::Missing,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultRoute {
    pub interface_index: u32,
    pub ipv6: bool,
    pub route_metric: u32,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub interface_luid: u64,
    pub name: String,
    pub interface_type: u32,
    pub operational_status: i32,
    pub receive_link_bits_per_second: u64,
    pub transmit_link_bits_per_second: u64,
    pub receive_bytes_per_second: Reading<u64>,
    pub transmit_bytes_per_second: Reading<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskSpace {
    pub drive: char,
    pub caller_available_bytes: u64,
    pub caller_total_bytes: u64,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnState {
    Unknown,
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Report {
    HostResources {
        interval_ms: u64,
        /// GetSystemTimes scope: current primary processor group, not necessarily every CPU.
        cpu_busy_basis_points: Reading<u16>,
        network: Reading<Vec<Network>>,
        /// Root of the Windows system directory's local drive, not the download target.
        system_drive_space: Reading<DiskSpace>,
        /// No app/disk utilization provider is guessed from unrelated counters.
        disk_pressure: Reading<u16>,
        #[serde(default)]
        default_routes: Reading<Vec<DefaultRoute>>,
        #[serde(default)]
        ipv4_dns_servers: Reading<u16>,
    },
    CiscoVpnStatus {
        state: Reading<VpnState>,
    },
}
impl Report {
    pub fn catalog(&self) -> Catalog {
        match self {
            Self::HostResources { .. } => Catalog::HostResources,
            Self::CiscoVpnStatus { .. } => Catalog::CiscoVpnStatus,
        }
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if let Self::HostResources {
            interval_ms,
            cpu_busy_basis_points,
            network,
            system_drive_space,
            disk_pressure,
            default_routes,
            ipv4_dns_servers,
        } = self
        {
            if matches!(default_routes,Reading::Available{value} if value.len()>64 || value.iter().any(|v|v.interface_index==0))
                || matches!(ipv4_dns_servers,Reading::Available{value} if *value>64)
            {
                return Err(ErrorCode::Malformed);
            }
            if !(500..=5000).contains(interval_ms)
                || matches!(cpu_busy_basis_points, Reading::Available { value } if *value > 10_000)
                || matches!(disk_pressure, Reading::Available { value } if *value > 10_000)
            {
                return Err(ErrorCode::Malformed);
            }
            if let Reading::Available { value } = network {
                if value.len() > 64
                    || value.iter().any(|v| {
                        v.interface_luid == 0
                            || v.name.len() > 96
                            || v.name.chars().any(char::is_control)
                    })
                {
                    return Err(ErrorCode::Malformed);
                }
                let mut ids = std::collections::HashSet::new();
                if value.iter().any(|v| !ids.insert(v.interface_luid)) {
                    return Err(ErrorCode::Malformed);
                }
            }
            if let Reading::Available { value } = system_drive_space
                && (!value.drive.is_ascii_uppercase()
                    || value.caller_available_bytes > value.caller_total_bytes)
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| ErrorCode::Malformed)?
            .len()
            > 30_000
        {
            return Err(ErrorCode::TooLarge);
        }
        Ok(())
    }
}
