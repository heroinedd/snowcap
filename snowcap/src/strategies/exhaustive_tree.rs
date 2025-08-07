//! # The Exhaustive Tree Strategy, visit all intermediate snapshots through a DFS/BFS traversal

use super::{ExhaustiveStrategy, Strategy};
use crate::hard_policies::HardPolicy;
use crate::modifier_ordering::ModifierOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network};
use crate::{Error, Stopper};

use log::*;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

/// # The Exhaustive Tree Strategy
///
/// The Tree strategy recursively builds a tree by choosing one of the remaining modifiers and
/// simulating the result. If all policies are satisfied, continue by choosing one of the remaining
/// modifiers. if nont of the remaining modifiers work, then fall back and declare this current
/// modifier as not-satisfying the policies.
///
/// ## Properties
///
/// This strategy benefits from problems with an *immediate effect*, since it can massively reduce
/// the search space if a problem is detected in an early stage of the tree. Thus, it is able to
/// find a solution of a `sparse problem` with *immediate effect* very quickly (`O(n^3)`). However,
/// it has problems when dependencies have *no immediate effect*.
///
/// ## Type Arguments
/// - `O` represents the chosen [`ModifierOrdering`](crate::modifier_ordering::ModifierOrdering),
///   which is used to order the modifiers before the tree algorithm starts.
pub struct ExhaustiveTreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    net: Network,
    modifiers: Vec<ConfigModifier>,
    stop_time: Option<SystemTime>,
    phantom: PhantomData<O>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<O> Strategy for ExhaustiveTreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    fn new(
        mut net: Network,
        mut modifiers: Vec<ConfigModifier>,
        mut _hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        // clear the undo stack
        net.clear_undo_stack();

        // sort the modifiers
        O::sort(&mut modifiers);

        trace!(
            "Modifiers:\n{}",
            modifiers
                .iter()
                .enumerate()
                .map(|(i, m)| format!("M{:02} {}", i, printer::config_modifier(&net, m).unwrap()))
                .collect::<Vec<String>>()
                .join("\n")
        );

        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            stop_time,
            phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        // initialize the stack
        let mut stack: Vec<Stack> = vec![Stack { rem_mod: self.modifiers.clone(), cur_idx: 0 }];
        let mut mod_sequence: Vec<ConfigModifier> = Vec::new();

        let mut net = self.net.clone();

        let mut all_valid_ordering: Vec<Vec<ConfigModifier>> = Vec::new();
        let mut num = 0;

        loop {
            num += 1;
            let mut pop_stack: bool = false;
            let mut push_stack: Option<Stack> = None;
            if let Some(s) = stack.last_mut() {
                // we are done if s.rem_mod is empty
                if s.rem_mod.is_empty() {
                    all_valid_ordering.push(mod_sequence.clone());
                    pop_stack = true;
                }
                // the current modifier is equal to the length of s.rem_mod!
                // the current modifier does not work, pop the stack!
                if s.cur_idx >= s.rem_mod.len() {
                    pop_stack = true;
                } else {
                    // try the current modifier
                    let cur_idx = s.cur_idx;
                    // move cur_idx to the next position for the next iteration
                    s.cur_idx += 1;
                    // get the current modifier and clone the current network
                    let current_mod: &ConfigModifier = &s.rem_mod[cur_idx];

                    // print the current sequence
                    if STATIC_MAX_LEVEL >= LevelFilter::Debug {
                        let mut print_vec: Vec<usize> = Vec::new();
                        for m in mod_sequence.iter() {
                            print_vec.push(self.modifiers.iter().position(|x| x == m).unwrap());
                        }
                        print_vec
                            .push(self.modifiers.iter().position(|x| x == current_mod).unwrap());
                        debug!("{:?}", print_vec);
                    }

                    // apply the modifier
                    #[cfg(feature = "count-states")]
                    {
                        self.num_states += 1;
                    }

                    let mod_ok = if net.apply_modifier(current_mod).is_ok() {
                        net.get_forwarding_state();
                        true
                    } else {
                        false
                    };

                    if mod_ok {
                        // this single modification works! continue with it
                        let mut new_mod = s.rem_mod.clone();
                        new_mod.remove(cur_idx);
                        push_stack = Some(Stack { rem_mod: new_mod, cur_idx: 0 });
                        mod_sequence.push(current_mod.clone());
                    } else {
                        net.undo_action()?;
                    }
                }
            } else {
                if !all_valid_ordering.is_empty() {
                    // println!("Number of all valid ordering: {:#?}", all_valid_ordering.len());
                    break Ok(all_valid_ordering[0].clone());
                }
                // the stack is empty! We found nothing!
                break Err(Error::NoSafeOrdering);
            }

            if pop_stack {
                // undo the network
                net.undo_action()?;
                // pop the stack
                stack.pop();
                mod_sequence.pop();
                debug!("Backtrack from tree, current levels: {}", stack.len());

                // check for time budget
                if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                    // time budget is used up!
                    error!("Time budget is used up! No solution was found yet!");
                    break Err(Error::Timeout);
                }

                // check for abort criteria
                if abort.try_is_stop().unwrap_or(false) {
                    info!("Operation was aborted!");
                    break Err(Error::Abort);
                }
            }

            if let Some(new_stack_element) = push_stack.take() {
                stack.push(new_stack_element);
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<O> ExhaustiveStrategy for ExhaustiveTreeStrategy<O> where O: ModifierOrdering<ConfigModifier> {}

struct Stack {
    pub rem_mod: Vec<ConfigModifier>,
    pub cur_idx: usize,
}
