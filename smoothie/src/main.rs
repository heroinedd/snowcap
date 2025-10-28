mod smoothie_chain;
mod test;
mod utils;

use crate::smoothie_chain::SmoothieChainGadget;
use crate::utils::*;
use glob::glob;
use log::error;
use serde::Serialize;
use snowcap::modifier_ordering::RandomOrdering;
use snowcap::netsim::config::ConfigPatch;
use snowcap::optimizers::{Optimizer, OptimizerTRTA};
use snowcap::permutators::RandomTreePermutator;
use snowcap::soft_policies::{MinimizeTrafficShift, SoftPolicy};
use snowcap::strategies::{
    ExhaustiveTreeStrategy, PermutationStrategy, Strategy, StrategyTRTA, TreeAllValidStrategy,
};
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

    // run TreeAllValidStrategy to find all valid ordering
    start = SystemTime::now();
    let mut permutator = TreeAllValidStrategy::<RandomOrdering>::new(
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

pub fn test_topology_zoo(scenario: Scenario) -> Result<(), Box<dyn Error>> {
    for e in glob("/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/*.gml")
        .expect("Failed to read glob pattern")
    {
        let path = e.unwrap();
        let file_path = path.to_str().unwrap();
        let file_name = file_path.split('/').last().unwrap();
        match run_topology_zoo(file_path, file_name, scenario.clone()) {
            Ok(_) => (),
            Err(e) => error!("{:?}", e),
        }
    }
    Ok(())
}

fn run_topology_zoo(
    file_path: &str,
    file_name: &str,
    scenario: Scenario,
) -> Result<(), Box<dyn Error>> {
    let mut zoo: ZooTopology = ZooTopology::new(file_path.to_string(), 0)?;
    let (network, final_config, hard_policy) =
        match zoo.apply_scenario(scenario.clone().into(), false, 100, 1, 1.0) {
            Ok((network, final_config, hard_policy)) => (network, final_config, hard_policy),
            Err(e) => return Err(Box::new(e)),
        };
    let patch: ConfigPatch = network.current_config().get_diff(&final_config);

    // run OptimizerTRTA
    let mut start = SystemTime::now();
    // let mut fw_state = network.get_forwarding_state();
    // let soft_policy = MinimizeTrafficShift::new(&mut fw_state, &network);
    // let mut optimizer = match OptimizerTRTA::<MinimizeTrafficShift>::new(
    //     network.clone(),
    //     patch.modifiers.clone(),
    //     hard_policy.clone(),
    //     soft_policy,
    //     Some(Duration::from_secs(600)),
    // ) {
    //     Ok(o) => o,
    //     Err(e) => return Err(Box::new(e)),
    // };
    // optimizer.work(Stopper::new())?;
    // let optimizer_duration = start.elapsed().unwrap().as_secs_f64();
    // print!("{:?}\t{:?}\t", file_name, optimizer_duration);

    // run StrategyTRTA to find one valid ordering
    let mut start = SystemTime::now();
    let mut trta = match StrategyTRTA::new(
        network.clone(),
        patch.modifiers.clone(),
        hard_policy.clone(),
        Some(Duration::from_secs(600)),
    ) {
        Ok(o) => o,
        Err(e) => return Err(Box::new(e)),
    };
    match trta.work(Stopper::new()) {
        Ok(_) => (),
        Err(e) => return Err(Box::new(e)),
    };
    let trta_duration = start.elapsed().unwrap().as_secs_f64();
    print!(
        "{:?}\t{:?}\t{:?}\t{:?}\n",
        scenario,
        file_name,
        trta_duration,
        trta.get_number_of_learned_dependencies()
    );

    // run ExhaustiveTreeStrategy to visit all intermediate snapshots
    // start = SystemTime::now();
    // let mut tree = TreeAllValidStrategy::<RandomOrdering>::new(
    //     network.clone(),
    //     patch.modifiers.clone(),
    //     hard_policy.clone(),
    //     Some(Duration::from_secs(600)),
    // )?;
    // tree.work(Stopper::new()).unwrap_or_default();
    // let tree_duration = start.elapsed().unwrap().as_secs_f64();
    // print!("{:?}\n", tree_duration);

    Ok(())
}

pub fn compare_snowcap_smoothie_schedules(topo_name: &str) -> Result<(), Box<dyn Error>> {
    let gml_file = format!(
        "/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/{}.gml",
        topo_name
    );

    for seed in 0..1 {
        let snowcap_cost = run_snowcap(gml_file.clone(), seed)?;
        let smoothie_cost = run_smoothie_schedule(gml_file.clone(), topo_name.to_string(), seed)?;
        println!("{}\t{}\t{}", seed, snowcap_cost, smoothie_cost);
    }

    Ok(())
}

fn run_snowcap(gml_file: String, seed: u64) -> Result<f64, Box<dyn Error>> {
    // run Snowcap
    let mut zoo: ZooTopology = ZooTopology::new(gml_file.clone(), seed)?;
    let (network, final_config, hard_policy) =
        zoo.apply_scenario(Scenario::DoubleIgpWeight.into(), false, 100, 1, 1.0)?;
    let patch: ConfigPatch = network.current_config().get_diff(&final_config);
    let mut fw_state = network.get_forwarding_state();
    let soft_policy = MinimizeTrafficShift::new(&mut fw_state, &network);
    let mut optimizer = OptimizerTRTA::<MinimizeTrafficShift>::new(
        network.clone(),
        patch.modifiers.clone(),
        hard_policy.clone(),
        soft_policy,
        Some(Duration::from_secs(600)),
    )?;
    let (_schedule, snowcap_cost) = optimizer.work(Stopper::new())?;
    Ok(snowcap_cost)
}

fn run_smoothie_schedule(
    gml_file: String,
    topo_name: String,
    seed: u64,
) -> Result<f64, Box<dyn Error>> {
    let mut zoo: ZooTopology = ZooTopology::new(gml_file.clone(), seed)?;
    let (mut network, _final_config, mut hard_policy) =
        zoo.apply_scenario(Scenario::DoubleIgpWeight.into(), false, 100, 1, 1.0)?;
    let modifiers = read_smoothie_schedule(&zoo.get_graph(), topo_name, seed)?;

    // apply ConfigModifiers and compute cost
    let mut fw_state = network.get_forwarding_state();
    let mut soft_policy = MinimizeTrafficShift::new(&mut fw_state, &network);
    let mut cost = 0.0;
    for modifier in modifiers {
        network
            .apply_modifier(&modifier)
            .expect("Modifier should be ok!");
        fw_state = network.get_forwarding_state();
        hard_policy
            .step(&mut network, &mut fw_state)
            .expect("Modifier should be ok!");
        soft_policy.update(&mut fw_state, &network);
        cost += soft_policy.cost();
    }
    Ok(cost)
}

#[derive(Debug, Clone, Serialize)]
struct TopologyZooResult {
    edges: HashMap<String, f32>,
    schedule: Vec<String>,
    cost: f64,
    learned_groups: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    // test_chain_change_steps()
    // test_chain_change_routers()
    // compare_snowcap_smoothie_schedules("Aconet")
    // run_topology_zoo(
    //     "/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/Aconet.gml",
    //     "Aconet.gml",
    //     Scenario::FullMesh2RouteReflector
    // )
    // Ok(write_acquisition())
    test_topology_zoo(Scenario::DoubleIgpWeight)?;
    test_topology_zoo(Scenario::FullMesh2RouteReflector)?;
    test_topology_zoo(Scenario::DoubleLocalPref)?;
    test_topology_zoo(Scenario::NetworkAcquisition)
}
