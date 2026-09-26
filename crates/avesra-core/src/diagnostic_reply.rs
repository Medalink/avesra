//! Deterministic explanations of typed native facts; no text from an external agent.
use crate::{
    diagnostics::{Reading, Report, VpnState},
    execution::EffectObservation,
};
use avesra_contracts::{Action, ActionPayload, ErrorCode, Outcome};

fn host(report: &Report) -> String {
    let Report::HostResources {
        interval_ms,
        disk_activity,
        cpu_busy_basis_points,
        network,
        default_routes,
        ipv4_dns_servers,
        ..
    } = report
    else {
        return String::new();
    };
    let cpu = match cpu_busy_basis_points {
        Reading::Available { value } => format!(
            "Processor-group CPU: {:.0} percent",
            f64::from(*value) / 100.0
        ),
        _ => "CPU unavailable".into(),
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
            "highest interface receive average: {:.2} megabytes per second, including other traffic",
            value as f64 / 1_000_000.0
        ),
        None => "network rates unavailable".into(),
    };
    let local = if matches!(default_routes,Reading::Available{value} if value.is_empty()) {
        "No default route observed. "
    } else if matches!(ipv4_dns_servers, Reading::Available { value: 0 }) {
        "No IPv4 DNS server configured. "
    } else {
        ""
    };
    let activity = match disk_activity {
        Reading::Available { value } => {
            let scope = match value.scope {
                crate::diagnostics::DiskScope::SystemVolume => "System volume",
                crate::diagnostics::DiskScope::SelectedDestination => "Destination volume",
            };
            let busy = match value.non_idle_basis_points {
                Reading::Available { value } => {
                    format!("{:.0} percent non-idle", f64::from(value) / 100.0)
                }
                Reading::Unavailable { .. } => "non-idle time unavailable".into(),
            };
            format!(
                "{scope} {}: {busy} over {:.2} seconds. ",
                value.drive,
                value.interval_ms as f64 / 1000.0,
            )
        }
        Reading::Unavailable { .. } => "Volume activity unavailable. ".into(),
    };
    format!(
        "Over {:.2} seconds: {cpu}; {network}. {local}{activity}",
        *interval_ms as f64 / 1000.0
    )
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
            Report::HostResources{..}=>format!("{}Cause unproven; nothing changed. Full measurements are in the report.",host(report)),
            Report::CiscoVpnStatus{state}=>match state {
                Reading::Available{value:VpnState::Connected}=>"Cisco reports a connected VPN. This status alone does not prove the selected work target or Spark is reachable.".into(),
                Reading::Available{value:VpnState::Disconnected}=>"Cisco reports the VPN is disconnected.".into(),
                Reading::Available{value:VpnState::Connecting|VpnState::Reconnecting}=>"Cisco reports the VPN is still connecting. I have not issued another connection.".into(),
                _=>"The current Cisco VPN state could not be established.".into(),
            },
        },
        Some(EffectObservation::Download{report})=>{
            let mut value=if report.dns_failure(){"IPv4 and IPv6 DNS failed. ".to_owned()}
                else if report.endpoints.is_empty(){"Endpoint resolution unavailable. ".into()}
                else {let successful=report.endpoints.iter().filter_map(|e|match e.tcp_connect_micros{Reading::Available{value}=>Some(value),_=>None}).min();
                    match successful{Some(micros)=>format!("Fastest TCP connection: {:.1} milliseconds. ",micros as f64/1000.0),None=>"Endpoint connection unavailable. ".into()}};
            value.push_str(&host(&report.resources));
            match &report.destination_space {
                Reading::Available { value: space } => value.push_str(&format!(
                    "Destination {}: {:.1} gigabytes available under your quota. ",
                    space.drive, space.caller_available_bytes as f64 / 1_000_000_000.0,
                )),
                Reading::Unavailable { .. } => value.push_str("Destination space unavailable. "),
            }
            value.push_str("Cause unproven; nothing changed. Full measurements are in the report.");value
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
    // Bounded concise summary, not a playback-duration guarantee; never truncate a fact.
    if text.len() > 1024 || !avesra_contracts::planner::valid_text(&text) {
        return Err(ErrorCode::TooLarge);
    }
    text.shrink_to_fit();
    Ok(text)
}
