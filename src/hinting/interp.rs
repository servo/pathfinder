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

use byteorder::{BigEndian, ByteOrder};
use error::{HintingAnalysisError, HintingExecutionError, HintingParseError};
use hinting::Hinter;
use hinting::insns::Instruction;

impl<'a> Hinter<'a> {
    pub fn exec(&mut self) -> Result<(), HintingExecutionError> {
        loop {
            // Fetch the current frame.
            let frame = match self.call_stack.last() {
                None => return Ok(()),
                Some(&frame) if frame.pc == frame.end => {
                    self.call_stack.pop();
                    continue
                }
                Some(&frame) => frame,
            };

            // Decode the next instruction, and advance the program counter.
            let mut new_pc = frame.pc;
            let bytecode = self.scripts[frame.script].bytecode;
            let instruction =
                try!(Instruction::parse(bytecode,
                                        &mut new_pc).map_err(HintingExecutionError::ParseError));

            // Execute it.
            match instruction {
                Instruction::Pushb(bytes) => {
                    self.stack.extend(bytes.iter().map(|&b| b as i32))
                }
                Instruction::Pushw(bytes) => {
                    self.stack.extend(bytes.chunks(2).map(|bs| BigEndian::read_i16(bs) as i32))
                }
                Instruction::Rs => {
                    // We should throw an exception here if the storage area isn't big enough, but
                    // let's follow Postel's law.
                    let addr = try!(self.pop()) as usize;
                    match self.storage_area.get(addr) {
                        Some(&value) => self.stack.push(value),
                        None => self.stack.push(0),
                    }
                }
                Instruction::Ws => {
                    // We should throw an exception here if the storage area isn't big enough, but
                    // let's follow Postel's law.
                    //
                    // FIXME(pcwalton): Cap the size of the storage area?
                    let (value, addr) = (try!(self.pop()), try!(self.pop()) as usize);
                    if self.storage_area.len() < addr + 1 {
                        self.storage_area.resize(addr + 1, 0)
                    }
                    self.storage_area[addr] = value
                }
                Instruction::Fdef => {
                    // We should throw an exception here if the function definition list isn't big
                    // enough, but let's follow Postel's law.
                    //
                    // FIXME(pcwalton): Cap the size of the function definitions?
                    let id = try!(self.pop()) as usize;
                    if self.functions.len() < id + 1 {
                        self.functions.resize(id + 1, None)
                    }

                    let branch_target_index = self.scripts[frame.script]
                                                  .branch_targets
                                                  .binary_search_by(|script| {
                                                    script.branch_location.cmp(&frame.pc)
                                                  }).unwrap();

                    let end_pc = self.scripts[frame.script]
                                     .branch_targets[branch_target_index]
                                     .target_location;

                    self.functions[id] = Some(Frame::new(new_pc, end_pc, frame.script));
                    new_pc = end_pc + 1
                }
                _ => {
                    println!("TODO: {:?}", instruction);
                }
            }

            // Advance the program counter.
            self.call_stack.last_mut().unwrap().pc = new_pc;
        }
    }

    #[inline]
    fn pop(&mut self) -> Result<i32, HintingExecutionError> {
        self.stack.pop().ok_or(HintingExecutionError::StackUnderflow)
    }
}

pub struct Script<'a> {
    bytecode: &'a [u8],
    branch_targets: Vec<BranchTarget>,
}

impl<'a> Script<'a> {
    pub fn new<'b>(bytecode: &'b [u8]) -> Result<Script<'b>, HintingAnalysisError> {
        let mut interpreter = Script {
            bytecode: bytecode,
            branch_targets: vec![],
        };
        try!(interpreter.populate_branch_targets());
        Ok(interpreter)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytecode.len()
    }

    // This is a little bit tricky because we have to maintain sorted order of the `branch_targets`
    // array for future binary searches.
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
                    pending_branch_targets.push((self.branch_targets.len(), instruction));
                    self.branch_targets.push(BranchTarget {
                        branch_location: location,
                        target_location: 0,
                    });
                }
                Instruction::Endf => {
                    let (index, branch_instruction) = try!(pending_branch_targets.pop().ok_or(
                            HintingAnalysisError::BranchTargetMissingBranch));
                    match branch_instruction {
                        Instruction::Fdef | Instruction::Idef => {
                            self.branch_targets[index].target_location = location
                        }
                        _ => return Err(HintingAnalysisError::MismatchedBranchInstruction),
                    }
                }
                Instruction::Eif => {
                    let (index, branch_instruction) = try!(pending_branch_targets.pop().ok_or(
                            HintingAnalysisError::BranchTargetMissingBranch));
                    match branch_instruction {
                        Instruction::If | Instruction::Else => {
                            self.branch_targets[index].target_location = location
                        }
                        _ => return Err(HintingAnalysisError::MismatchedBranchInstruction),
                    }
                }
                Instruction::Else => {
                    let (index, branch_instruction) = try!(pending_branch_targets.pop().ok_or(
                            HintingAnalysisError::BranchTargetMissingBranch));
                    match branch_instruction {
                        Instruction::If => {
                            self.branch_targets[index].target_location = location;

                            pending_branch_targets.push((self.branch_targets.len(), instruction));
                            self.branch_targets.push(BranchTarget {
                                branch_location: location,
                                target_location: 0,
                            });
                        }
                        _ => return Err(HintingAnalysisError::MismatchedBranchInstruction),
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
pub struct Frame {
    /// The current program counter.
    pc: usize,
    /// The PC at which to stop execution.
    end: usize,
    /// The index of the script.
    script: usize,
}

impl Frame {
    pub fn new(pc: usize, end: usize, script_index: usize) -> Frame {
        Frame {
            pc: pc,
            end: end,
            script: script_index,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BranchTarget {
    branch_location: usize,
    target_location: usize,
}

