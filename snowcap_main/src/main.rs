// Snowcap: Synthesizing Network-Wide Configuration Updates
// Copyright (C) 2021  Tibor Schneider
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

use snowcap::hard_policies::*;
use snowcap::netsim::{config::Config, printer, Network, NetworkError};
use snowcap::optimizers::*;
use snowcap::permutators::*;
use snowcap::soft_policies::*;
use snowcap::strategies::*;
use snowcap::topology_zoo::{self, ZooTopology};
use snowcap::{optimize, synthesize, Stopper};
use snowcap_bencher::*;
use snowcap_runtime::perform_migration;

use clap::Clap;
use log::*;
use rand::prelude::*;
use std::error::Error;
use std::fmt;

mod example_topologies;
use example_topologies::*;
mod transient_violation;
use transient_violation::*;
mod arguments;
use arguments::*;
mod utils;
use utils::*;

fn main() -> Result<(), Box<dyn Error>> {
    // run clap
    let args = CommandLineArguments::parse();

    // match on the action
    match args.cmd {
        MainCommand::TransientViolation {
            gml_file,
            seed,
            n_seeds,
            n_iter,
            reverse,
            num_threads,
        } => {
            transient_violation_topologyzoo(gml_file, seed, n_seeds, n_iter, num_threads, reverse)?
        }
        MainCommand::CustomOperation { n_iter, variant } => transient_violation(n_iter, variant)?,
        MainCommand::Optimize { network, use_tree } => {
            // initialize the env logger
            pretty_env_logger::init();
            // get the network
            let (net, final_config, hard_policy) = get_topo(network)?;
            check_config(&net, &final_config)?;
            let initial_config = net.current_config().clone();

            info!(
                "Problem has {} modifiers",
                initial_config.get_diff(&final_config).modifiers.len()
            );

            let mut fw_state = net.get_forwarding_state();
            let soft_policy = MinimizeTrafficShift::new(&mut fw_state, &net);

            // generate the update sequence
            info!("Generating the update sequence");
            let (sequence, cost) = if use_tree {
                TreeOptimizer::<_>::synthesize(
                    net.clone(),
                    final_config,
                    hard_policy,
                    soft_policy,
                    None,
                    Stopper::new(),
                )?
            } else {
                optimize::<MinimizeTrafficShift>(
                    net.clone(),
                    initial_config,
                    final_config,
                    hard_policy,
                    None,
                )?
            };

            info!(
                "Update sequence with cost: {}:\n    {}",
                cost,
                sequence
                    .iter()
                    .map(|m| printer::config_modifier(&net, m).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );
        }
        MainCommand::Synthesize { network, use_tree } => {
            // initialize the env logger
            pretty_env_logger::init();
            // get the network
            let (net, final_config, hard_policy) = get_topo(network)?;
            check_config(&net, &final_config)?;
            let initial_config = net.current_config().clone();

            info!(
                "Problem has {} modifiers",
                initial_config.get_diff(&final_config).modifiers.len()
            );

            // generate the update sequence
            info!("Generating the update sequence");
            let sequence = if use_tree {
                PermutationStrategy::<RandomTreePermutator>::synthesize(
                    net.clone(),
                    final_config,
                    hard_policy,
                    None,
                    Stopper::new(),
                )?
            } else {
                synthesize(
                    net.clone(),
                    initial_config,
                    final_config,
                    hard_policy,
                    Some(std::time::Duration::from_secs(3600)),
                )?
            };

            info!(
                "Update sequence:\n    {}",
                sequence
                    .iter()
                    .map(|m| printer::config_modifier(&net, m).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );
        }
        MainCommand::Runtime {
            network,
            persistent_gns_project,
            random_sequence,
            at_once,
            seed,
            json_filename,
        } => {
            // initialize the env logger
            pretty_env_logger::init();
            // get the network
            let (net, final_config, hard_policy) = get_topo(network)?;
            check_config(&net, &final_config)?;
            let initial_config = net.current_config().clone();

            let sequence = if random_sequence {
                info!("Generating a random update sequence");
                let mut s = initial_config.get_diff(&final_config).modifiers;
                if let Some(seed) = seed {
                    let mut rng = StdRng::seed_from_u64(seed);
                    s.shuffle(&mut rng);
                } else {
                    s.shuffle(&mut thread_rng());
                }
                s
            } else {
                // generate the update sequence
                info!("Generating the update sequence");
                synthesize(
                    net.clone(),
                    initial_config,
                    final_config,
                    hard_policy,
                    Some(std::time::Duration::from_secs(3600)),
                )?
            };

            info!(
                "Update sequence:\n    {}",
                sequence
                    .iter()
                    .map(|m| printer::config_modifier(&net, m).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );

            perform_migration(
                &net,
                &sequence,
                persistent_gns_project,
                json_filename,
                at_once,
            )?;
        }
        MainCommand::Bencher { network, args } => {
            let scenario = network.repr();
            let (net, final_config, hard_policy) = get_topo(network)?;
            bench(net, final_config, hard_policy, scenario, args)?;
        }
    }
    Ok(())
}

/// This is the binary to use the runtime systen esily. This program will generate the topology and
/// the reconfiguration scenario (based on the options provided), synthesize a reconfiguration order
/// and perform this order on a network simulated inside GNS3 using FRRouting.
#[derive(Clap, Debug)]
#[clap(name = "Runtime (Binary)", author = "Tibor Schneider")]
struct CommandLineArguments {
    /// Action to perform
    #[clap(subcommand)]
    cmd: MainCommand,
}
