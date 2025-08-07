use snowcap::netsim::config::{ConfigExpr, ConfigModifier};
use snowcap::netsim::printer::route_map;
use snowcap::netsim::route_map::RouteMapDirection;
use snowcap::netsim::{BgpSessionType, Network, NetworkError};

/// Returns a formatted string for the given modifier, where all router names are inserted.
pub fn config_modifier(net: &Network, modifier: &ConfigModifier) -> Result<String, NetworkError> {
    Ok(match modifier {
        ConfigModifier::Insert(e) => format!("INSERT {}", config_expr(net, e)?),
        ConfigModifier::Remove(e) => format!("REMOVE {}", config_expr(net, e)?),
        ConfigModifier::Update { from: _, to } => format!("MODIFY {}", config_expr(net, to)?),
    })
}

/// Returns the config expr as a string, where all router names are inserted.
pub fn config_expr(net: &Network, expr: &ConfigExpr) -> Result<String, NetworkError> {
    Ok(match expr {
        ConfigExpr::IgpLinkWeight {
            source,
            target,
            weight,
        } => format!(
            "IGP Link Weight: {} -> {}: {}",
            source.index(),
            target.index(),
            weight
        ),
        ConfigExpr::BgpSession {
            source,
            target,
            session_type,
        } => format!(
            "BGP Session: {} -> {}: type: {}",
            source.index(),
            target.index(),
            match session_type {
                BgpSessionType::EBgp => "eBGP",
                BgpSessionType::IBgpClient => "iBGP Client",
                BgpSessionType::IBgpPeer => "iBGP Peer",
            }
        ),
        ConfigExpr::BgpRouteMap {
            router,
            direction,
            map,
        } => format!(
            "BGP Route Map on {} [{}]: {}",
            router.index(),
            match direction {
                RouteMapDirection::Incoming => "in",
                RouteMapDirection::Outgoing => "out",
            },
            route_map(net, map)?
        ),
        ConfigExpr::StaticRoute {
            router,
            prefix,
            target,
        } => format!(
            "Static Route: {}: Prefix {} via {}",
            router.index(),
            prefix.0,
            target.index()
        ),
    })
}
