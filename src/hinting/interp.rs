// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The TrueType interpreter.

use error::{HintingAnalysisError, HintingParseError};
use hinting::insns::Instruction;

pub struct ScriptInterpreter<'a> {
    bytecode: &'a [u8],
    branch_targets: Vec<BranchTarget>,
}

impl<'a> ScriptInterpreter<'a> {
    pub fn new<'b>(bytecode: &'b [u8]) -> Result<ScriptInterpreter<'b>, HintingAnalysisError> {
        let mut interpreter = ScriptInterpreter {
            bytecode: bytecode,
            branch_targets: vec![],
        };
        try!(interpreter.populate_branch_targets());
        Ok(interpreter)
    }

    fn populate_branch_targets(&mut self) -> Result<(), HintingAnalysisError> {
        let (mut pc, mut pending_branch_targets) = (0, vec![]);
        loop {
            let location = pc;
            let instruction = match Instruction::parse(self.bytecode, &mut pc) {
                Ok(instruction) => instruction,
                Err(HintingParseError::Eof) => break,
                Err(err) => return Err(HintingAnalysisError::ParseError(err)),
            };

            match instruction {
                Instruction::If | Instruction::Fdef | Instruction::Idef => {
                    pending_branch_targets.push((location, instruction))
                }
                Instruction::Endf => {
                    match pending_branch_targets.pop() {
                        Some((branch_location, Instruction::Fdef)) |
                        Some((branch_location, Instruction::Idef)) => {
                            self.branch_targets.push(BranchTarget {
                                branch_location: branch_location,
                                target_location: location,
                            })
                        }
                        Some(_) => return Err(HintingAnalysisError::MismatchedBranchInstruction),
                        None => return Err(HintingAnalysisError::BranchTargetMissingBranch),
                    }
                }
                Instruction::Eif => {
                    match pending_branch_targets.pop() {
                        Some((branch_location, Instruction::If)) |
                        Some((branch_location, Instruction::Else)) => {
                            self.branch_targets.push(BranchTarget {
                                branch_location: branch_location,
                                target_location: location,
                            })
                        }
                        Some(_) => return Err(HintingAnalysisError::MismatchedBranchInstruction),
                        None => return Err(HintingAnalysisError::BranchTargetMissingBranch),
                    }
                }
                Instruction::Else => {
                    match pending_branch_targets.pop() {
                        Some((branch_location, Instruction::If)) => {
                            self.branch_targets.push(BranchTarget {
                                branch_location: branch_location,
                                target_location: location,
                            });
                            pending_branch_targets.push((location, instruction))
                        }
                        Some(_) => return Err(HintingAnalysisError::MismatchedBranchInstruction),
                        None => return Err(HintingAnalysisError::BranchTargetMissingBranch),
                    }
                }
                _ => {}
            }
        }

        if pending_branch_targets.is_empty() {
            Ok(())
        } else {
            Err(HintingAnalysisError::BranchMissingBranchTarget)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BranchTarget {
    branch_location: usize,
    target_location: usize,
}

