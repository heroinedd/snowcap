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

//! Soft Policy to prefer a specific ordering between two modifiers (u1 before u2).

use super::SoftPolicy;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{ForwardingState, Network};

/// # Soft Policy: Prefer Ordering
///
/// This soft policy penalizes orderings where `u2` is applied before `u1`. If `u2` appears in
/// the event history before `u1`, a cost of 1.0 is returned; otherwise 0.0.
#[derive(Clone, Debug)]
pub struct PreferOrdering {
    u1: ConfigModifier,
    u2: ConfigModifier,
    violation: bool,
}

impl PreferOrdering {
    /// Create a new `PreferOrdering` policy that prefers `u1` to be applied before `u2`.
    pub fn new_with_modifiers(u1: ConfigModifier, u2: ConfigModifier) -> Self {
        Self { u1, u2, violation: false }
    }
}

impl SoftPolicy for PreferOrdering {
    fn new(_state: &mut ForwardingState, _net: &Network) -> Self {
        panic!("PreferOrdering requires modifiers; use PreferOrdering::new_with_modifiers instead")
    }

    fn update(&mut self, _state: &mut ForwardingState, net: &Network) {
        if self.violation {
            return;
        }
        let mut u1_seen = false;
        for m in net.applied_modifiers() {
            if m == &self.u1 {
                u1_seen = true;
            }
            if m == &self.u2 && !u1_seen {
                self.violation = true;
                return;
            }
        }
    }

    fn cost(&self) -> f64 {
        if self.violation {
            1.0
        } else {
            0.0
        }
    }
}
