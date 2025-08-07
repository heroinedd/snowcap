use snowcap::hard_policies::HardPolicy;
use snowcap::modifier_ordering::RandomOrdering;
use snowcap::netsim::config::ConfigPatch;
use snowcap::netsim::Network;
use snowcap::permutators::RandomTreePermutator;
use snowcap::strategies::{ExhaustiveTreeStrategy, PermutationStrategy, Strategy, StrategyTRTA};
use snowcap::Stopper;
use snowcap_main::arguments::NetworkSelection;
use snowcap_main::example_topologies::Reps;
use snowcap_main::example_topologies::Reps::*;
use snowcap_main::example_topologies::Topology::{ChainKGadget, ChainNGadget};
use snowcap_main::utils::{check_config, get_topo};
use std::error::Error;
use std::time::{Duration, SystemTime};

pub fn test_chain_k() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    for rep in vec![
        Rep10, Rep20, Rep30, Rep40, Rep50, Rep60, Rep70, Rep80, Rep90, Rep100, Rep200, Rep300,
        Rep400, Rep500,
    ] {
        let network: NetworkSelection = NetworkSelection::ExampleNetwork {
            initial_variant: 0,
            final_variant: Some(0),
            repetitions: Some(rep),
            topology: ChainKGadget,
        };
        let (net, final_config, hard_policy) = get_topo(network)?;
        check_config(&net, &final_config)?;
        let initial_config = net.current_config().clone();
        let patch: ConfigPatch = initial_config.get_diff(&final_config);

        run_strategies(rep, net, patch, hard_policy)?;
    }
    Ok(())
}

pub fn test_chain_n() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    for rep in vec![Rep100, Rep200, Rep300, Rep400, Rep500] {
        let network: NetworkSelection = NetworkSelection::ExampleNetwork {
            initial_variant: 0,
            final_variant: Some(0),
            repetitions: Some(rep),
            topology: ChainNGadget,
        };
        let (net, final_config, hard_policy) = get_topo(network)?;
        check_config(&net, &final_config)?;
        let initial_config = net.current_config().clone();
        let patch: ConfigPatch = initial_config.get_diff(&final_config);

        run_strategies(rep, net, patch, hard_policy)?;
    }
    Ok(())
}

fn run_strategies(
    rep: Reps,
    net: Network,
    patch: ConfigPatch,
    hard_policy: HardPolicy,
) -> Result<(), Box<dyn Error>> {
    print!("{:?}\t", rep);

    // run StrategyTRTA to find one valid ordering
    let mut start = SystemTime::now();
    let mut trta = StrategyTRTA::new(
        net.clone(),
        patch.modifiers.clone(),
        hard_policy.clone(),
        None,
    )?;
    trta.work(Stopper::new())?;
    let trta_duration = start.elapsed().unwrap().as_secs_f64();
    print!("{:?}\t", trta_duration);

    // run PermutationStrategy to find all valid ordering
    start = SystemTime::now();
    let mut permutator = PermutationStrategy::<RandomTreePermutator>::new(
        net.clone(),
        patch.modifiers.clone(),
        hard_policy.clone(),
        Some(Duration::from_secs(600)),
    )?;
    permutator.work(Stopper::new()).unwrap_or_default();
    let permutator_duration = start.elapsed().unwrap().as_secs_f64();
    print!("{:?}\t", permutator_duration);

    // run ExhaustiveTreeStrategy to visit all intermediate snapshots
    start = SystemTime::now();
    let mut tree = ExhaustiveTreeStrategy::<RandomOrdering>::new(
        net.clone(),
        patch.modifiers.clone(),
        hard_policy.clone(),
        Some(Duration::from_secs(600)),
    )?;
    tree.work(Stopper::new()).unwrap_or_default();
    let tree_duration = start.elapsed().unwrap().as_secs_f64();
    print!("{:?}\t", tree_duration);

    println!();
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    test_chain_k().unwrap_or_default();
    test_chain_n()
}
