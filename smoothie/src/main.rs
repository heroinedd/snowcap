use clap::Clap;
use log::info;
use snowcap::hard_policies::HardPolicy;
use snowcap::modifier_ordering::RandomOrdering;
use snowcap::netsim::config::Config;
use snowcap::netsim::{printer, Network, NetworkError};
use snowcap::soft_policies::MinimizeTrafficShift;
use snowcap::strategies::{StrategyTRTA, TreeStrategy};
use snowcap::topology_zoo::ZooTopology;
use snowcap::{synthesize, synthesize_parallel, Stopper};
use snowcap_bencher::runner_strategy::worker_runner;
use snowcap_bencher::BencherType::Strategy;
use snowcap_bencher::{bench, BencherArguments};
use snowcap_main::arguments::{NetworkSelection, Scenario};
use snowcap_main::example_topologies::Reps::Rep10;
use snowcap_main::example_topologies::Topology::{ChainGadget, ChainKGadget};
use snowcap_main::example_topologies::{example_networks_scenario, Reps, Topology};
use snowcap_main::utils::{check_config, get_topo};
use std::error::Error;
use std::time::Instant;

pub fn test_chain() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let network: NetworkSelection = NetworkSelection::ExampleNetwork {
        initial_variant: 0,
        final_variant: Some(0),
        repetitions: Some(Rep10),
        topology: ChainGadget,
    };
    let scenario = network.repr();
    let (net, final_config, hard_policy) = get_topo(network)?;
    check_config(&net, &final_config)?;
    let initial_config = net.current_config().clone();

    info!(
        "Problem has {} modifiers",
        initial_config.get_diff(&final_config).modifiers.len()
    );

    let mut start = Instant::now();
    info!("synthesize parallel");
    synthesize_parallel(
        net.clone(),
        initial_config,
        final_config.clone(),
        hard_policy.clone(),
        std::time::Duration::from_secs(3600),
        Some(1),
    )?;
    let mut parallel_duration = start.elapsed();
    info!("Parallel duration: {:?}", parallel_duration);

    start = Instant::now();
    info!("synthesize bench");
    let args: BencherArguments = BencherArguments {
        bench_type: Strategy,
        iterations: 1,
        max_time: 300,
        ignore_nan: true,
        random: false,
        tree: false,
        main: true,
        mif: false,
        mil: false,
        global_optimum: false,
        threads: Some(10),
        output_csv: None,
        output_json: None,
    };
    worker_runner::<StrategyTRTA, MinimizeTrafficShift>(
        &net,
        &final_config,
        &hard_policy,
        args.max_time,
        args.iterations,
        args.ignore_nan,
        1,
    );
    let bench_duration = start.elapsed();
    info!("Bench duration: {:?}", bench_duration);

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    test_chain()
}
