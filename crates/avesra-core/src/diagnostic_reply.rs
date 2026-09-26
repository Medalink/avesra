//! Deterministic explanations of typed native facts; no text from an external agent.
use crate::{
    diagnostics::{Reading, Report, VpnState},
    execution::EffectObservation,
};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome};

fn host(report: &Report) -> String {
    let Report::HostResources {
        cpu_busy_basis_points,
        network,
        system_drive_space,
        default_routes,
        ipv4_dns_servers,
        ..
    } = report
    else {
        return String::new();
    };
    let cpu = match cpu_busy_basis_points {
        Reading::Available { value } => format!(
            "CPU usage for the measured processor group was {:.0} percent",
            f64::from(*value) / 100.0
        ),
        _ => "CPU usage was unavailable".into(),
    };
    let rate = if let Reading::Available { value } = network {
        value
            .iter()
            .filter_map(|v| match v.receive_bytes_per_second {
                Reading::Available { value } => Some(value),
                _ => None,
            })
            .max()
    } else {
        None
    };
    let network = match rate {
        Some(value) => format!(
            "the busiest interface averaged {:.2} megabytes per second, including other traffic",
            value as f64 / 1_000_000.0
        ),
        None => "network rates were unavailable".into(),
    };
    let local = if matches!(default_routes,Reading::Available{value} if value.is_empty()) {
        "no default route was observed".into()
    } else if matches!(ipv4_dns_servers, Reading::Available { value: 0 }) {
        "no configured IPv4 DNS server was observed".into()
    } else if let Reading::Available { value } = system_drive_space {
        format!(
            "system-drive free space was {:.1} gigabytes",
            value.caller_available_bytes as f64 / 1_000_000_000.0
        )
    } else {
        "system-drive space was unavailable".into()
    };
    format!("{cpu}; {network}; {local}. ")
}

pub(crate) fn describe(
    action: &Action,
    outcome: Outcome,
    observation: Option<&EffectObservation>,
) -> Result<String, ErrorCode> {
    if !matches!(
        action.payload,
        ActionPayload::Diagnostic { .. }
            | ActionPayload::DiagnoseDownload { .. }
            | ActionPayload::FlushDownloadDns { .. }
            | ActionPayload::ConnectVpn { .. }
    ) {
        return Err(ErrorCode::Unsupported);
    }
    if let Some(observation) = observation {
        observation.validate(action, outcome)?;
    }
    let mut text=match observation {
        Some(EffectObservation::Diagnostic{report})=>match report.as_ref() {
            Report::HostResources{..}=>format!("{}The bottleneck, app rate, throttle and destination remain unknown; select the download endpoint for a connection check.",host(report)),
            Report::CiscoVpnStatus{state}=>match state {
                Reading::Available{value:VpnState::Connected}=>"Cisco reports a connected VPN. This status alone does not prove the selected work target or Spark is reachable.".into(),
                Reading::Available{value:VpnState::Disconnected}=>"Cisco reports the VPN is disconnected.".into(),
                Reading::Available{value:VpnState::Connecting|VpnState::Reconnecting}=>"Cisco reports the VPN is still connecting. I have not issued another connection.".into(),
                _=>"The current Cisco VPN state could not be established.".into(),
            },
        },
        Some(EffectObservation::Download{report})=>{
            let mut value=if report.dns_failure(){"Both address families returned DNS failures. The cause, including stale cache, is unproven. ".to_owned()}
                else if report.endpoints.is_empty(){"Endpoint resolution was unavailable or timed out; that does not justify clearing DNS cache. ".into()}
                else {let successful=report.endpoints.iter().filter_map(|e|match e.tcp_connect_micros{Reading::Available{value}=>Some(value),_=>None}).min();
                    match successful{Some(micros)=>format!("The fastest successful TCP connection took {:.1} milliseconds; this is not download throughput. ",micros as f64/1000.0),None=>"The endpoint connection checks did not complete successfully; the cause remains unknown. ".into()}};
            value.push_str("App throughput, throttling, disk utilization and server capacity remain unmeasured. Nothing was changed.");value
        },
        Some(EffectObservation::DnsFlush{command_completed:true,..}) if outcome==Outcome::Success=>"The approved Windows DNS-cache flush command completed. This does not prove that stale cache caused the problem or that the download is fixed. Ask me to diagnose the download again to obtain fresh observations.".into(),
        Some(EffectObservation::DnsFlush{..})=>"The DNS-cache command's outcome is uncertain. I will not retry it automatically or claim the problem is fixed.".into(),
        Some(EffectObservation::Vpn{report})=>match report.state {
            crate::vpn::State::AlreadyConnected=>"Cisco already reports a connection to the selected work VPN. I did not issue a connection command. VPN state alone does not prove Spark reachability.".into(),
            crate::vpn::State::Connected=>"Cisco now reports a connection to the selected work VPN. VPN state alone does not prove Spark reachability.".into(),
            crate::vpn::State::OtherConnection=>"Cisco reports another VPN connection. I did not replace or disconnect it.".into(),
            crate::vpn::State::NeedsOwner=>"Cisco needs your attention to finish the connection. Complete authentication or review its prompt in Cisco; I do not enter credentials or accept security prompts.".into(),
            crate::vpn::State::Uncertain=>"The VPN connection outcome is unresolved. Inspect Cisco before another connection attempt; I have not retried or disconnected it.".into(),
        },
        None=>match outcome {
            Outcome::UnknownEffect=>"The native operation ended with an uncertain outcome. I cannot confirm the requested result and will not retry automatically.".into(),
            Outcome::NeedsInput=>"The native operation requires your attention. No measured result is available; inspect its task status.".into(),
            Outcome::Failed|Outcome::Unsupported=>"The native operation could not produce a supported measurement. No result has been inferred.".into(),
            _=>return Err(ErrorCode::Unsupported),
        },
        _=>return Err(ErrorCode::Unsupported),
    };
    // Less than one bounded short spoken response; never truncate a fact.
    if text.len() > 1024 || !avesra_contracts::planner::valid_text(&text) {
        return Err(ErrorCode::TooLarge);
    }
    text.shrink_to_fit();
    Ok(text)
}
