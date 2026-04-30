mod smoothie_chain;
mod utils;

use crate::smoothie_chain::SmoothieChainGadget;
use crate::utils::*;
use glob::glob;
use log::error;
use serde::Serialize;
use snowcap::example_networks::{ExampleNetwork, SimpleNet};
use snowcap::hard_policies::HardPolicy;
use snowcap::modifier_ordering::RandomOrdering;
use snowcap::netsim::config::ConfigPatch;
use snowcap::optimizers::{Optimizer, OptimizerTRTA, TreeOptimizer};
use snowcap::permutators::RandomTreePermutator;
use snowcap::soft_policies::{MinimizeTrafficShift, PreferOrdering, SoftPolicy};
use snowcap::strategies::{
    ExhaustiveTreeStrategy, PermutationStrategy, Strategy, StrategyTRTA, TreeAllValidStrategy,
};
use snowcap::topology_zoo::ZooTopology;
use snowcap::Stopper;
use snowcap_main::arguments::Scenario;
use std::collections::HashMap;
use std::env;
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

pub fn test_topology_zoo(
    scenario: Scenario,
    num_prefixes_per_er: usize,
    run_optimizer: bool,
    run_trta: bool,
    run_exhaustive: bool,
    transient: bool,
    repeats: usize,
) -> Result<(), Box<dyn Error>> {
    for e in glob("/Users/wangdan/ANTS/snowcap/eval_sigcomm2021/topology_zoo/*.gml")
        .expect("Failed to read glob pattern")
    {
        let path = e.unwrap();
        let file_path = path.to_str().unwrap();
        let file_name = file_path.split('/').last().unwrap();
        for _ in 0..repeats {
            match run_topology_zoo(
                file_path,
                file_name,
                scenario.clone(),
                num_prefixes_per_er,
                run_optimizer,
                run_trta,
                run_exhaustive,
                transient,
            ) {
                Ok(_) => (),
                Err(e) => error!("{:?}", e),
            }
        }
    }
    Ok(())
}

fn run_topology_zoo(
    file_path: &str,
    file_name: &str,
    scenario: Scenario,
    num_prefixes_per_er: usize,
    run_optimizer: bool,
    run_trta: bool,
    run_exhaustive: bool,
    transient: bool,
) -> Result<(), Box<dyn Error>> {
    let mut zoo: ZooTopology = ZooTopology::new(file_path.to_string(), 0)?;

    let (network, final_config, hard_policy, initial_time) = match zoo
        .apply_scenario_record_initial_time(
            scenario.clone().into(),
            false,
            100,
            num_prefixes_per_er,
            1.0,
            transient,
        ) {
        Ok((network, final_config, hard_policy, initial_time)) => {
            (network, final_config, hard_policy, initial_time)
        }
        Err(e) => return Err(Box::new(e)),
    };

    let patch: ConfigPatch = network.current_config().get_diff(&final_config);
    let num_updates = patch.modifiers.len();

    // run OptimizerTRTA with PreferOrdering on a random pair of updates
    if run_optimizer {
        let mut start = SystemTime::now();
        let n = patch.modifiers.len();
        let (i, j) = if n >= 2 {
            use rand::seq::index::sample;
            let mut rng = rand::thread_rng();
            let s = sample(&mut rng, n, 2);
            (s.index(0), s.index(1))
        } else {
            (0, 0)
        };
        let soft_policy = PreferOrdering::new_with_modifiers(
            patch.modifiers[i].clone(),
            patch.modifiers[j].clone(),
        );
        let mut optimizer = match OptimizerTRTA::<PreferOrdering>::new(
            network.clone(),
            patch.modifiers.clone(),
            hard_policy.clone(),
            soft_policy,
            Some(Duration::from_secs(600)),
        ) {
            Ok(o) => o,
            Err(e) => return Err(Box::new(e)),
        };
        optimizer.work(Stopper::new())?;
        let optimizer_duration = start.elapsed().unwrap().as_secs_f64();
        print!(
            "{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\n",
            scenario,
            file_name,
            zoo.ibgp_roots,
            initial_time,
            optimizer_duration,
            optimizer_duration / initial_time,
            optimizer.num_states(),
            num_updates
        );
    }

    // run StrategyTRTA to find one valid ordering
    if run_trta {
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
        let plan = trta.work(Stopper::new())?;
        let plan_str: Vec<String> = plan
            .iter()
            .map(|s| config_modifier(&network, s).unwrap())
            .collect::<Vec<String>>();
        let trta_duration = start.elapsed().unwrap().as_secs_f64() - trta.get_prop_check_time();
        print!(
            "{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\n",
            scenario,
            file_name,
            zoo.ibgp_roots,
            initial_time,
            trta_duration,
            trta_duration / initial_time,
            trta.num_states(),
            num_updates,
            trta.get_number_of_learned_dependencies()
        );
        // println!("{:?}", plan_str);
    }

    // run ExhaustiveTreeStrategy to visit all intermediate snapshots
    if run_exhaustive {
        let mut start = SystemTime::now();
        let mut tree = TreeAllValidStrategy::<RandomOrdering>::new(
            network.clone(),
            patch.modifiers.clone(),
            hard_policy.clone(),
            Some(Duration::from_secs(600)),
        )?;
        tree.work(Stopper::new()).unwrap_or_default();
        let tree_duration = start.elapsed().unwrap().as_secs_f64();
        print!("{:?}\n", tree_duration);
    }

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

/// Test whether Snowcap's TreeOptimizer can find a reconfiguration plan that satisfies a
/// `PreferOrdering(u1, u2)` soft constraint (i.e., u1 applied before u2). Uses SimpleNet as the
/// test network and exercises every consecutive modifier pair in both orderings.
pub fn test_prefer_ordering() -> Result<(), Box<dyn Error>> {
    let net = SimpleNet::net(0);
    let final_config = SimpleNet::final_config(&net, 0);
    let hard_policy = SimpleNet::get_policy(&net, 0);
    let patch = net.current_config().get_diff(&final_config);
    let modifiers = patch.modifiers;

    println!("SimpleNet: {} modifiers", modifiers.len());
    for (i, m) in modifiers.iter().enumerate() {
        println!("  m{}: {}", i, config_modifier(&net, m)?);
    }
    println!();

    for i in 0..modifiers.len().saturating_sub(1) {
        for (label, u1_idx, u2_idx) in [("forward", i, i + 1), ("reverse", i + 1, i)] {
            let u1 = modifiers[u1_idx].clone();
            let u2 = modifiers[u2_idx].clone();
            let policy = PreferOrdering::new_with_modifiers(u1, u2);

            let result = TreeOptimizer::<PreferOrdering>::new(
                net.clone(),
                modifiers.clone(),
                hard_policy.clone(),
                policy,
                None,
            )
            .and_then(|mut opt| opt.work(Stopper::new()));

            match result {
                Ok((ordering, cost)) => {
                    // cost == 0 means u1 was always applied before u2
                    let pos_u1 = ordering.iter().position(|m| m == &modifiers[u1_idx]);
                    let pos_u2 = ordering.iter().position(|m| m == &modifiers[u2_idx]);
                    println!(
                        "PreferOrdering(m{}, m{}) [{}]: plan found | cost={:.1} | \
                         preference {} | m{} at pos {:?}, m{} at pos {:?}",
                        u1_idx,
                        u2_idx,
                        label,
                        cost,
                        if cost == 0.0 { "SATISFIED" } else { "VIOLATED" },
                        u1_idx,
                        pos_u1,
                        u2_idx,
                        pos_u2,
                    );
                }
                Err(e) => {
                    println!(
                        "PreferOrdering(m{}, m{}) [{}]: no plan found — {:?}",
                        u1_idx, u2_idx, label, e
                    );
                }
            }
        }
        println!();
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "test_prefer_ordering") {
        return test_prefer_ordering();
    }

    // Parse named flags: --num-prefix N  --mode optimizer|trta|exhaustive  --repeats N
    let mut num_prefix = 1usize;
    let mut run_optimizer = false;
    let mut run_trta = false;
    let mut run_exhaustive = false;
    let mut repeats = 1usize;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--num-prefix" => {
                i += 1;
                num_prefix = args[i].parse()?;
            }
            "--mode" => {
                i += 1;
                match args[i].as_str() {
                    "optimizer" => run_optimizer = true,
                    "trta" => run_trta = true,
                    "exhaustive" => run_exhaustive = true,
                    m => {
                        return Err(format!(
                            "unknown mode: {m} (expected optimizer|trta|exhaustive)"
                        )
                        .into())
                    }
                }
            }
            "--repeats" => {
                i += 1;
                repeats = args[i].parse()?;
            }
            flag => return Err(format!("unknown argument: {flag}").into()),
        }
        i += 1;
    }
    test_topology_zoo(
        Scenario::DoubleIgpWeight,
        num_prefix,
        run_optimizer,
        run_trta,
        run_exhaustive,
        false,
        repeats,
    )?;
    test_topology_zoo(
        Scenario::FullMesh2RouteReflector,
        num_prefix,
        run_optimizer,
        run_trta,
        run_exhaustive,
        false,
        repeats,
    )?;
    test_topology_zoo(
        Scenario::DoubleLocalPref,
        num_prefix,
        run_optimizer,
        run_trta,
        run_exhaustive,
        false,
        repeats,
    )?;
    test_topology_zoo(
        Scenario::NetworkAcquisition,
        num_prefix,
        run_optimizer,
        run_trta,
        run_exhaustive,
        false,
        repeats,
    )?;
    test_topology_zoo(
        Scenario::DoubleRouteReflector,
        num_prefix,
        run_optimizer,
        run_trta,
        run_exhaustive,
        false,
        repeats,
    )
}
