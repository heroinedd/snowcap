mod smoothie_chain;
mod utils;

use crate::smoothie_chain::SmoothieChainGadget;
use glob::glob;
use serde::ser::SerializeMap;
use serde::Serialize;
use snowcap::modifier_ordering::RandomOrdering;
use snowcap::netsim::config::ConfigPatch;
use snowcap::optimizers::{Optimizer, OptimizerTRTA};
use snowcap::permutators::RandomTreePermutator;
use snowcap::soft_policies::{MinimizeTrafficShift, SoftPolicy};
use snowcap::strategies::{ExhaustiveTreeStrategy, PermutationStrategy, Strategy, StrategyTRTA};
use snowcap::topology_zoo::ZooTopology;
use snowcap::Stopper;
use snowcap_main::arguments::Scenario;
use std::collections::HashMap;
use std::error::Error;
use std::time::{Duration, SystemTime};

pub fn test_chain_change_steps() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    for rep in 1..11 {
        let chain: SmoothieChainGadget = SmoothieChainGadget::new(100, rep);
        run_strategies(rep, &chain)?;
    }
    Ok(())
}

pub fn test_chain_change_routers() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    for rep in (10..=100).step_by(10) {
        let chain: SmoothieChainGadget = SmoothieChainGadget::new(rep, 10);
        run_strategies(rep, &chain)?;
    }
    Ok(())
}

pub fn test_topology_zoo() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    for e in glob("/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/*.gml")
        .expect("Failed to read glob pattern")
    {
        let mut path = e.unwrap();
        let file_path = path.to_str().unwrap();
        let file_name = file_path.split('/').last().unwrap();

        let mut zoo: ZooTopology = ZooTopology::new(file_path.to_string(), 0)?;
        let (network, final_config, hard_policy) =
            zoo.apply_scenario(Scenario::DoubleIgpWeight.into(), false, 100, 1, 1.0)?;
        let patch: ConfigPatch = network.current_config().get_diff(&final_config);

        // run OptimizerTRTA
        let mut start = SystemTime::now();
        let mut fw_state = network.get_forwarding_state();
        let soft_policy = MinimizeTrafficShift::new(&mut fw_state, &network);
        let mut optimizer = OptimizerTRTA::<MinimizeTrafficShift>::new(
            network.clone(),
            patch.modifiers.clone(),
            hard_policy.clone(),
            soft_policy,
            Some(Duration::from_secs(600)),
        )?;
        let (mut schedule, cost) = optimizer.work(Stopper::new())?;
        let optimizer_duration = start.elapsed().unwrap().as_secs_f64();
        print!("{:?}\t{:?}\t", file_name, optimizer_duration);

        // run ExhaustiveTreeStrategy to visit all intermediate snapshots
        start = SystemTime::now();
        let mut tree = ExhaustiveTreeStrategy::<RandomOrdering>::new(
            network.clone(),
            patch.modifiers.clone(),
            hard_policy.clone(),
            Some(Duration::from_secs(60)),
        )?;
        tree.work(Stopper::new()).unwrap_or_default();
        let tree_duration = start.elapsed().unwrap().as_secs_f64();
        print!("{:?}\n", tree_duration);

        // output results to json
        let mut graph_str: HashMap<String, f32> = HashMap::<String, f32>::new();
        let graph = zoo.get_graph();
        graph.raw_edges().iter().for_each(|e| {
            let src = e.source();
            let dst = e.target();
            let w = e.weight.abs();
            graph_str.insert(format!("{} | {}", src.index(), dst.index()), w);
        });
        let schedule_str: Vec<String> = schedule
            .iter()
            .map(|m| utils::config_modifier(&network, m).unwrap())
            .collect();
        let learned_groups = optimizer.num_groups();
        let result = TopologyZooResult {
            edges: graph_str,
            schedule: schedule_str,
            cost,
            learned_groups,
        };
        let result_str = serde_json::to_string_pretty(&result)?;
        std::fs::write(
            format!(
                "/Users/wangdan/ANTS/snowcap/smoothie/topology_zoo/{}.json",
                file_name
            ),
            result_str,
        )?;
        break;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct TopologyZooResult {
    edges: HashMap<String, f32>,
    schedule: Vec<String>,
    cost: f64,
    learned_groups: usize,
}

fn run_strategies(rep: i32, chain: &SmoothieChainGadget) -> Result<(), Box<dyn Error>> {
    print!("{:?}\t", rep);

    let net = &chain.network;
    let patch: ConfigPatch = chain.initial_config.get_diff(&chain.final_config);
    let hard_policy = &chain.hard_policy;

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
    // test_chain_change_steps()
    //     .unwrap_or_default();
    // test_chain_change_routers()
    test_topology_zoo()
}
