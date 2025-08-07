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

//! # Chain Gadget
//! This topology consists of a single chain (line) of `N` routers. on each end of the line, we have
//! two border routers, which both receive the same advertisement. The reconfiguration requires the
//! routers to be reconfigured from the right to the left. There is only one single valid solution.

use snowcap::hard_policies::HardPolicy;
use snowcap::netsim::config::Config;
use snowcap::netsim::config::ConfigExpr::{BgpRouteMap, BgpSession, IgpLinkWeight};
use snowcap::netsim::route_map::{RouteMapBuilder, RouteMapDirection};
use snowcap::netsim::BgpSessionType::{EBgp, IBgpPeer};
use snowcap::netsim::{AsId, Network, Prefix};

/// A modified chain gadget

pub struct SmoothieChainGadget {
    pub num_routers: i32,
    pub num_steps: i32,
    pub network: Network,
    pub initial_config: Config,
    pub final_config: Config,
    pub hard_policy: HardPolicy,
}

impl SmoothieChainGadget {
    pub fn new(num_routers: i32, num_steps: i32) -> SmoothieChainGadget {
        let network: Network = Self::net(num_routers);
        let initial_config: Config = network.current_config().clone();
        let final_config: Config = Self::final_config(&network, num_routers, num_steps);
        let hard_policy: HardPolicy = Self::get_policy(&network);
        SmoothieChainGadget {
            num_routers,
            num_steps,
            network,
            initial_config,
            final_config,
            hard_policy,
        }
    }

    fn net(num_routers: i32) -> Network {
        let mut net = Network::new();

        let e0 = net.add_external_router(String::from("e00"), AsId(65100));
        let e1 = net.add_external_router(String::from("e01"), AsId(65101));
        let b0 = net.add_router(String::from("b00"));
        let b1 = net.add_router(String::from("b01"));

        net.add_link(e0, b0);
        net.add_link(e1, b1);

        let mut current_r = net.add_router(String::from("r00"));
        let mut last_r = b0;
        net.add_link(current_r, last_r);

        for i in 1..num_routers {
            last_r = current_r;
            current_r = net.add_router(format!("r{:02}", i));
            net.add_link(current_r, last_r);
        }

        net.add_link(current_r, b1);

        // apply initial config
        let cf = Self::initial_config(&net, num_routers);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(e0, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, num_routers: i32) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight {
            source: e0,
            target: b0,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: b0,
            target: e0,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: e1,
            target: b1,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: b1,
            target: e1,
            weight: 1.0,
        })
        .unwrap();
        c.add(BgpSession {
            source: e0,
            target: b0,
            session_type: EBgp,
        })
        .unwrap();
        c.add(BgpSession {
            source: e1,
            target: b1,
            session_type: EBgp,
        })
        .unwrap();

        // set the local pref
        c.add(BgpRouteMap {
            router: b0,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e0)
                .set_local_pref(200)
                .build(),
        })
        .unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight {
            source: current_r,
            target: last_r,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            target: current_r,
            source: last_r,
            weight: 1.0,
        })
        .unwrap();

        for i in 1..num_routers {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight {
                source: current_r,
                target: last_r,
                weight: 1.0,
            })
            .unwrap();
            c.add(IgpLinkWeight {
                target: current_r,
                source: last_r,
                weight: 1.0,
            })
            .unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight {
            source: current_r,
            target: last_r,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            target: current_r,
            source: last_r,
            weight: 1.0,
        })
        .unwrap();

        // set all other bgp sessions
        for i in 0..num_routers {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession {
                source: r,
                target: b1,
                session_type: IBgpPeer,
            })
            .unwrap();
        }

        c
    }

    fn final_config(net: &Network, num_routers: i32, num_steps: i32) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight {
            source: e0,
            target: b0,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: b0,
            target: e0,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: e1,
            target: b1,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            source: b1,
            target: e1,
            weight: 1.0,
        })
        .unwrap();
        c.add(BgpSession {
            source: e0,
            target: b0,
            session_type: EBgp,
        })
        .unwrap();
        c.add(BgpSession {
            source: e1,
            target: b1,
            session_type: EBgp,
        })
        .unwrap();

        // set the local pref
        c.add(BgpRouteMap {
            router: b0,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e0)
                .set_local_pref(200)
                .build(),
        })
        .unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight {
            source: current_r,
            target: last_r,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            target: current_r,
            source: last_r,
            weight: 1.0,
        })
        .unwrap();

        for i in 1..num_routers {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight {
                source: current_r,
                target: last_r,
                weight: 1.0,
            })
            .unwrap();
            c.add(IgpLinkWeight {
                target: current_r,
                source: last_r,
                weight: 1.0,
            })
            .unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight {
            source: current_r,
            target: last_r,
            weight: 1.0,
        })
        .unwrap();
        c.add(IgpLinkWeight {
            target: current_r,
            source: last_r,
            weight: 1.0,
        })
        .unwrap();

        // set all other bgp sessions
        for i in 0..num_routers {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession {
                source: r,
                target: b1,
                session_type: IBgpPeer,
            })
            .unwrap();
        }
        for i in 0..num_steps {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession {
                source: r,
                target: b0,
                session_type: IBgpPeer,
            })
            .unwrap();
        }

        c
    }

    fn get_policy(net: &Network) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
