use std::error::Error;
use snowcap::hard_policies::HardPolicy;
use snowcap::netsim::config::Config;
use snowcap::netsim::{Network, NetworkError};
use snowcap::topology_zoo::ZooTopology;
use crate::arguments::{NetworkSelection, Scenario};
use crate::example_topologies::example_networks_scenario;

pub fn get_topo(args: NetworkSelection) -> Result<(Network, Config, HardPolicy), Box<dyn Error>> {
    match args {
        NetworkSelection::CustomNetwork => custom_scenario(),
        NetworkSelection::TopologyZoo {
            gml_file,
            seed,
            many_prefixes,
            random_root,
            scenario,
        } => topology_zoo_scenario(gml_file, seed, many_prefixes, random_root, scenario),
        NetworkSelection::ExampleNetwork {
            topology,
            initial_variant,
            final_variant,
            repetitions,
        } => example_networks_scenario(topology, initial_variant, final_variant, repetitions),
    }
}

pub fn custom_scenario() -> Result<(Network, Config, HardPolicy), Box<dyn Error>> {
    todo!()
}

pub fn topology_zoo_scenario(
    gml_file: String,
    seed: u64,
    many_prefixes: bool,
    random_root: bool,
    scenario: Scenario,
) -> Result<(Network, Config, HardPolicy), Box<dyn Error>> {
    Ok(ZooTopology::new(&gml_file, seed)?.apply_scenario(
        scenario.into(),
        random_root,
        100,
        if many_prefixes { 5 } else { 1 },
        if many_prefixes { 0.5 } else { 1.0 },
    )?)
}

pub fn check_config(net: &Network, final_config: &Config) -> Result<(), Box<dyn Error>> {
    match net.clone().set_config(final_config) {
        Ok(()) => Ok(()),
        Err(NetworkError::ConvergenceLoop(_, _)) => Ok(()),
        Err(NetworkError::NoConvergence) => Ok(()),
        Err(e) => Err(e.into()),
    }
}