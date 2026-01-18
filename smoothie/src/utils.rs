use glob::glob;
use log::error;
use petgraph::graph::NodeIndex;
use petgraph::{Graph, Undirected};
use serde::{Deserialize, Serialize};
use snowcap::netsim::config::ConfigExpr::IgpLinkWeight;
use snowcap::netsim::config::ConfigModifier::Update;
use snowcap::netsim::config::{ConfigExpr, ConfigModifier};
use snowcap::netsim::printer::route_map;
use snowcap::netsim::route_map::RouteMapDirection;
use snowcap::netsim::{BgpSessionType, LinkWeight, Network, NetworkError, RouterId};
use snowcap::topology_zoo::{NodeData, ZooTopology};
use std::collections::HashMap;
use std::error::Error;

/// Returns a formatted string for the given modifier, where all router names are inserted.
pub fn config_modifier(net: &Network, modifier: &ConfigModifier) -> Result<String, NetworkError> {
    Ok(match modifier {
        ConfigModifier::Insert(e) => format!("+ {}", config_expr(net, e)?),
        ConfigModifier::Remove(e) => format!("- {}", config_expr(net, e)?),
        ConfigModifier::Update { from: _, to } => format!("m {}", config_expr(net, to)?),
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
            "{} Session: {} -> {}",
            match session_type {
                BgpSessionType::EBgp => "eBGP",
                BgpSessionType::IBgpClient => "iBGP Client",
                BgpSessionType::IBgpPeer => "iBGP Peer",
            },
            source.index(),
            target.index(),
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

pub fn generate_topology_zoo_edge_weights() -> Result<(), Box<dyn Error>> {
    for e in glob("/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/*.gml")
        .expect("Failed to read glob pattern")
    {
        let path = e.unwrap();
        let file_path = path.to_str().unwrap();
        let file_name = file_path.split('/').last().unwrap();
        let name = file_name.split('.').next().unwrap();

        let mut weights = HashMap::<u64, HashMap<String, f32>>::new();
        for seed in 0..10 {
            let mut zoo: ZooTopology = ZooTopology::new(file_path.to_string(), seed)?;
            zoo.randomize_link_weights(100);

            // output results to json
            let mut weights_with_seed: HashMap<String, f32> = HashMap::<String, f32>::new();
            let graph = zoo.get_graph();
            graph.raw_edges().iter().for_each(|e| {
                let src = e.source();
                let dst = e.target();
                let w = e.weight.abs();
                weights_with_seed.insert(format!("{} | {}", src.index(), dst.index()), w);
            });
            weights.insert(seed, weights_with_seed);
        }

        let result_str = serde_json::to_string_pretty(&weights)?;
        std::fs::write(
            format!(
                "/Users/wangdan/ANTS/smoothie/networks/topology-zoo/{}/{}.json",
                name, name
            ),
            result_str,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DoubleIgpLinkWeight {
    source: usize,
    target: usize,
    weight: f32,
}

impl DoubleIgpLinkWeight {
    pub(crate) fn to_config_modifier(&self) -> ConfigModifier {
        Update {
            from: IgpLinkWeight {
                source: RouterId::new(self.source),
                target: RouterId::new(self.target),
                weight: self.weight,
            },
            to: IgpLinkWeight {
                source: RouterId::new(self.source),
                target: RouterId::new(self.target),
                weight: self.weight * 2.0,
            },
        }
    }

    pub(crate) fn check_edge_weight(
        &self,
        graph: &Graph<NodeData, LinkWeight, Undirected, u32>,
    ) -> bool {
        let edge = match graph.find_edge(NodeIndex::new(self.source), NodeIndex::new(self.target)) {
            Some(e) => e,
            None => graph
                .find_edge(NodeIndex::new(self.target), NodeIndex::new(self.source))
                .unwrap(),
        };
        graph.edge_weight(edge).unwrap().abs() == self.weight
    }
}

pub(crate) fn read_smoothie_schedule(
    graph: &Graph<NodeData, LinkWeight, Undirected, u32>,
    topo_name: String,
    seed: u64,
) -> Result<Vec<ConfigModifier>, Box<dyn Error>> {
    let filename = format!(
        "/Users/wangdan/ANTS/smoothie/networks/topology-zoo/{}/IGPx2/out/schedule_{}.json",
        topo_name, seed
    );
    let schedule: Vec<DoubleIgpLinkWeight> =
        serde_json::from_reader(std::fs::File::open(filename)?)?;

    // check link weights consistency
    if schedule.iter().any(|m| !m.check_edge_weight(&graph)) {
        error!("link weights do not match");
    }

    // transfer to ConfigModifiers in Snowcap
    let modifiers: Vec<ConfigModifier> = schedule.iter().map(|m| m.to_config_modifier()).collect();
    Ok(modifiers)
}

pub fn write_acquisition() {
    for e in glob("/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/*.gml")
        .expect("Failed to read glob pattern")
    {
        let path = e.unwrap();
        let file_path = path.to_str().unwrap();
        write_acquisition_subnets(file_path);
    }
}

pub fn write_acquisition_subnets(file_path: &str) {
    let file_name = file_path.split('/').last().unwrap();
    let name = file_name.split('.').next().unwrap();
    println!("Writing acquisition of {}", name);

    let mut acqs: HashMap<u64, Vec<Component>> = HashMap::new();
    for seed in 0..10 {
        let mut zoo: ZooTopology = ZooTopology::new(file_path.to_string(), seed).unwrap();
        match zoo.acquisition_before(0.1) {
            Ok(_) => {}
            Err(_) => return,
        }

        // nodes
        let mut nodes1: Vec<usize> = Vec::new();
        let mut nodes2: Vec<usize> = Vec::new();
        zoo.get_graph()
            .node_indices()
            .into_iter()
            .filter(|n| !zoo.get_graph().node_weight(*n).unwrap().external)
            .for_each(|n| {
                if zoo.get_disconnected().contains(&n.index()) {
                    nodes2.push(n.index());
                } else {
                    nodes1.push(n.index());
                }
            });

        // rrs
        let rrs = zoo.get_ibgp_roots();
        let rr1: usize = *rrs.get(0).unwrap();
        let rr2: usize = *rrs.get(1).unwrap();
        let flag = nodes1.contains(&rr1);

        let mut components: Vec<Component> = Vec::new();
        components.push(Component {
            nodes: nodes1,
            rr: if flag { rr1 } else { rr2 },
        });
        components.push(Component {
            nodes: nodes2,
            rr: if flag { rr2 } else { rr1 },
        });
        acqs.insert(seed, components);
    }

    let result_str = serde_json::to_string_pretty(&acqs).unwrap();
    let write_result = std::fs::write(
        format!(
            "/Users/wangdan/ANTS/smoothie/networks/topology-zoo/{}/subnetworks.json",
            name
        ),
        result_str,
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Component {
    nodes: Vec<usize>,
    rr: usize,
}
