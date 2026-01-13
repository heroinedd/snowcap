use crate::example_topologies::{Reps, Topology};
use clap::Clap;
use snowcap::topology_zoo;
use snowcap_bencher::BencherArguments;
use std::fmt;

#[derive(Clap, Debug)]
pub enum MainCommand {
    /// Perform the migration synthesis
    #[clap(name = "synthesize")]
    Synthesize {
        /// Use the tree strategy instead of the more complex one
        #[clap(short = 't', long)]
        use_tree: bool,
        /// Type of measurement to perform
        #[clap(subcommand)]
        network: NetworkSelection,
    },
    /// Perform the migration synthesis using soft policies
    #[clap(name = "optimize")]
    Optimize {
        /// Use the tree strategy instead of the more complex one
        #[clap(short = 't', long)]
        use_tree: bool,
        /// Type of measurement to perform
        #[clap(subcommand)]
        network: NetworkSelection,
    },
    /// Perform the migration synthesis and the Runtime simulation
    #[clap(name = "run")]
    Runtime {
        /// Type of measurement to perform
        #[clap(subcommand)]
        network: NetworkSelection,
        /// Leave the gns3 project open after quitting this project
        #[clap(short = 'p', long)]
        persistent_gns_project: bool,
        /// Use a random sequence for the reconfiguration
        #[clap(short = 'r', long)]
        random_sequence: bool,
        /// Apply all modifiers at once, without monitoring the network
        #[clap(short = 'a', long)]
        at_once: bool,
        /// Seed for the random sequence (if used)
        #[clap(short = 's', long)]
        seed: Option<u64>,
        /// Store the result summary in a json file
        #[clap(long = "json")]
        json_filename: Option<String>,
    },
    /// Run the Bencher
    #[clap(name = "bench")]
    Bencher {
        /// Type of measurement to perform
        #[clap(subcommand)]
        network: NetworkSelection,
        /// Bencher Arguments
        #[clap(flatten)]
        args: BencherArguments,
    },
    /// Verify transient condition and violations
    #[clap(name = "transient")]
    TransientViolation {
        /// GML file to use for topology zoo
        gml_file: String,
        /// Random seed, to get reproducable networks
        #[clap(short = 's', long, default_value = "42")]
        seed: u64,
        /// Number of iterations for approximating probability of transient violation
        #[clap(short = 'n', long, default_value = "10000")]
        n_iter: usize,
        /// Number of different seeds to evaluate, repeating the same experiment multiple times
        #[clap(short = 'i', long, default_value = "100")]
        n_seeds: usize,
        /// perform the modification in reverse (remove the eBGP peer)
        #[clap(short = 'r', long)]
        reverse: bool,
        /// Number of parallel execution units
        #[clap(long)]
        num_threads: Option<usize>,
    },
    /// Custom method
    #[clap(name = "custom")]
    CustomOperation {
        /// Number of iterations for computing
        #[clap(short = 'n', long, default_value = "10000")]
        n_iter: usize,
        /// Variant of the scenario to compute the violation
        #[clap(short = 'v', long, default_value = "1")]
        variant: usize,
    },
}

/// This is the binary to use the runtime systen esily. This program will generate the topology and
/// the reconfiguration scenario (based on the options provided), synthesize a reconfiguration order
/// and perform this order on a network simulated inside GNS3 using FRRouting.
#[derive(Clap, Debug)]
pub enum NetworkSelection {
    /// Use the custom hard-coded network
    #[clap(name = "custom")]
    CustomNetwork,
    /// Use the network from Topology Zoo
    #[clap(name = "topology-zoo")]
    TopologyZoo {
        /// GML file to use
        gml_file: String,
        /// Random seed, to get reproducable networks
        #[clap(short = 's', long, default_value = "42")]
        seed: u64,
        /// use many prefixes (i.e., 5 prefixes, distributed with a probability of 0.5)
        #[clap(short = 'm', long)]
        many_prefixes: bool,
        /// Use a random roots when generating configuration
        #[clap(short = 'r', long)]
        random_root: bool,
        /// Select the reconfiguration scenario
        #[clap(arg_enum)]
        scenario: Scenario,
    },
    /// Use an example network, provided by snowcap
    #[clap(name = "example")]
    ExampleNetwork {
        /// Initial variant
        #[clap(short = 'i', long, default_value = "0")]
        initial_variant: usize,
        /// Final variant
        #[clap(short = 'f', long)]
        final_variant: Option<usize>,
        /// Repetitions (does not affect every variant)
        #[clap(arg_enum, short = 'r', long)]
        repetitions: Option<Reps>,
        /// Topology to use (from the example topologies)
        #[clap(arg_enum)]
        topology: Topology,
    },
}

impl NetworkSelection {
    /// Stringify the network
    pub fn repr(&self) -> String {
        match self {
            NetworkSelection::CustomNetwork => "Custom Network".to_string(),
            NetworkSelection::TopologyZoo {
                gml_file,
                many_prefixes,
                random_root,
                scenario,
                seed,
            } => {
                format!(
                    "{}, s={}, {}{}{}",
                    gml_file.split('/').collect::<Vec<_>>().last().unwrap(),
                    seed,
                    scenario,
                    if !many_prefixes {
                        ", single-prefix"
                    } else {
                        ""
                    },
                    if *random_root { ", random-root" } else { "" },
                )
            }
            NetworkSelection::ExampleNetwork {
                initial_variant,
                final_variant,
                repetitions,
                topology,
            } => format!(
                "{}{}{}{}",
                topology,
                if *initial_variant != 0 {
                    format!(", initial_variant={}", initial_variant)
                } else {
                    "".to_string()
                },
                if let Some(f) = final_variant {
                    format!(", final_variant={}", f)
                } else {
                    "".to_string()
                },
                if let Some(r) = repetitions {
                    format!(", rep={}", r)
                } else {
                    "".to_string()
                }
            ),
        }
    }
}

#[derive(Clap, Debug, Clone)]
pub enum Scenario {
    /// Scenario, where we start with a iBGP full mesh, and end up with a topology, where one single
    /// router is elected as a Route Reflectors, and all others pair with that router.
    #[clap(name = "FM2RR")]
    FullMesh2RouteReflector,
    /// Scenario, where we start with a topology, where one single router is elected as a Route
    /// Reflectors, and all others pair with that router, and we end up wiht an iBGP full mesh.
    #[clap(name = "RR2FM")]
    RouteReflector2FullMesh,
    /// Scenario, where every IGP weight is doubled
    #[clap(name = "IGPx2")]
    DoubleIgpWeight,
    /// Scenario, where every IGP weight is halved
    #[clap(name = "IGPdiv2")]
    HalveIgpWeight,
    /// Scenario, where every loacl pref is doubled
    #[clap(name = "LPx2")]
    DoubleLocalPref,
    /// Scenario, where every local pref is halved
    #[clap(name = "LPdiv2")]
    HalveLocalPref,
    /// Scenario, where we start with a single Route-Reflector, to which all other routers pair, and
    /// end with a second Route-Reflector as a backup, where all other routers have a session to
    /// both reflectors, and the two reflectors are connected with a peer.
    #[clap(name = "add2ndRR")]
    IntroduceSecondRouteReflector,
    /// Scenario, where we start with a second Route-Reflector as a backup, where all other routers
    /// have a session to both reflectors, and the two reflectors are connected with a peer, and end
    /// with a single Route-Reflector, to which all other routers pair.
    #[clap(name = "del2ndRR")]
    RemoveSecondRouteReflector,
    /// Scenario, where we start with two different connected components, both having connection to
    /// the outside world, and we merge them by adding the links in between.
    #[clap(name = "NetAcq")]
    NetworkAcquisition,
    /// Reverse scenario of the Network Acquisition
    #[clap(name = "NetSplit")]
    NetworkSplit,
    /// Disconnect a random non-border router form the network by setting all of its link weights to
    /// infinity. The IBGP topoogy will be a Route-Reflector topology, and the router disabled will
    /// not be selected as root!
    #[clap(name = "DiscR")]
    DisconnectRouter,
    /// Connect a random non-border router to the network by setting all of its link weights to a
    /// normal number. The IBGP topoogy will be a Route-Reflector topology, and the router disabled
    /// will not be selected as root!
    #[clap(name = "ConnR")]
    ConnectRouter,
    /// Test scenario for verifying transient state conditions. This scenario contains only a single
    /// modifier, which adds an eBGP session.
    #[clap(name = "Transient")]
    VerifyTransientCondition,
    /// Test scenario for verifying transient state conditions. This scenario contains only a single
    /// modifier, which adds an eBGP session.
    #[clap(name = "TransientRev")]
    VerifyTransientConditionReverse,
    DoubleRouteReflector,
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scenario::FullMesh2RouteReflector => {
                write!(f, "FullMesh2RouteReflector")
            }
            Scenario::RouteReflector2FullMesh => {
                write!(f, "RouteReflector2FullMesh")
            }
            Scenario::DoubleIgpWeight => {
                write!(f, "DoubleIgpWeight")
            }
            Scenario::HalveIgpWeight => {
                write!(f, "HalveIgpWeight")
            }
            Scenario::DoubleLocalPref => {
                write!(f, "DoubleLocalPref")
            }
            Scenario::HalveLocalPref => {
                write!(f, "HalveLocalPref")
            }
            Scenario::IntroduceSecondRouteReflector => {
                write!(f, "IntroduceSecondRouteReflector")
            }
            Scenario::RemoveSecondRouteReflector => {
                write!(f, "RemoveSecondRouteReflector")
            }
            Scenario::NetworkAcquisition => {
                write!(f, "NetworkAcquisition")
            }
            Scenario::NetworkSplit => {
                write!(f, "NetworkSplit")
            }
            Scenario::DisconnectRouter => {
                write!(f, "DisconnectRouter")
            }
            Scenario::ConnectRouter => {
                write!(f, "ConnectRouter")
            }
            Scenario::VerifyTransientCondition => {
                write!(f, "VerifyTransientCondition")
            }
            Scenario::VerifyTransientConditionReverse => {
                write!(f, "VerifyTransientConditionReverse")
            }
            Scenario::DoubleRouteReflector => {
                write!(f, "DoubleRouteReflector")
            }
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<topology_zoo::Scenario> for Scenario {
    fn into(self) -> topology_zoo::Scenario {
        match self {
            Scenario::FullMesh2RouteReflector => topology_zoo::Scenario::FullMesh2RouteReflector,
            Scenario::RouteReflector2FullMesh => topology_zoo::Scenario::RouteReflector2FullMesh,
            Scenario::DoubleIgpWeight => topology_zoo::Scenario::DoubleIgpWeight,
            Scenario::HalveIgpWeight => topology_zoo::Scenario::HalveIgpWeight,
            Scenario::DoubleLocalPref => topology_zoo::Scenario::DoubleLocalPref,
            Scenario::HalveLocalPref => topology_zoo::Scenario::HalveLocalPref,
            Scenario::IntroduceSecondRouteReflector => {
                topology_zoo::Scenario::IntroduceSecondRouteReflector
            }
            Scenario::RemoveSecondRouteReflector => {
                topology_zoo::Scenario::RemoveSecondRouteReflector
            }
            Scenario::NetworkAcquisition => topology_zoo::Scenario::NetworkAcquisition,
            Scenario::NetworkSplit => topology_zoo::Scenario::NetworkSplit,
            Scenario::DisconnectRouter => topology_zoo::Scenario::DisconnectRouter,
            Scenario::ConnectRouter => topology_zoo::Scenario::ConnectRouter,
            Scenario::VerifyTransientCondition => topology_zoo::Scenario::VerifyTransientCondition,
            Scenario::VerifyTransientConditionReverse => {
                topology_zoo::Scenario::VerifyTransientConditionReverse
            }
            Scenario::DoubleRouteReflector => topology_zoo::Scenario::DoubleRouteReflector,
        }
    }
}
