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
use hinting::insns::Instruction;
use hinting::{FONT_SMOOTHING_GRAYSCALE, GETINFO_VERSION, Hinter};
use hinting::{INFO_RESULT_FONT_SMOOTHING_GRAYSCALE_SHIFT, InfoSelector, RoundState, VERSION};
use num_traits::Zero;
use std::cmp;
use util::{F26DOT6_ZERO, F26Dot6};

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
                Instruction::Rcvt => {
                    let addr = try!(self.pop()) as usize;
                    let value = *try!(self.control_value_table
                                          .get(addr)
                                          .ok_or(HintingExecutionError::CvtReadOutOfBounds));
                    self.stack.push(value.0)
                }
                Instruction::Svtca(axis) => {
                    self.projection_vector = axis.as_point();
                    self.freedom_vector = axis.as_point();
                }
                Instruction::Spvtca(axis) => self.projection_vector = axis.as_point(),
                Instruction::Sfvtca(axis) => self.freedom_vector = axis.as_point(),
                Instruction::Srp0 => self.reference_points[0] = try!(self.pop()) as u32,
                Instruction::Srp1 => self.reference_points[1] = try!(self.pop()) as u32,
                Instruction::Srp2 => self.reference_points[2] = try!(self.pop()) as u32,
                Instruction::Szp0 => self.zone_points[0] = try!(self.pop()) as u32,
                Instruction::Szp1 => self.zone_points[1] = try!(self.pop()) as u32,
                Instruction::Szp2 => self.zone_points[2] = try!(self.pop()) as u32,
                Instruction::Szps => {
                    let zone = try!(self.pop()) as u32;
                    self.zone_points = [zone; 3]
                }
                Instruction::Rthg => self.round_state = RoundState::RoundToHalfGrid,
                Instruction::Rtg => self.round_state = RoundState::RoundToGrid,
                Instruction::Rtdg => self.round_state = RoundState::RoundToDoubleGrid,
                Instruction::Rutg => self.round_state = RoundState::RoundUpToGrid,
                Instruction::Roff => self.round_state = RoundState::RoundOff,
                Instruction::Sround => {
                    // TODO(pcwalton): Super rounding.
                    try!(self.pop());
                }
                Instruction::Scanctrl | Instruction::Scantype => {
                    // Not applicable to antialiased glyphs.
                    try!(self.pop());
                }
                Instruction::Mppem => {
                    // We always scale both axes in the same direction, so we don't have to look
                    // at the projection vector.
                    self.stack.push(self.point_size.round() as i32)
                }
                Instruction::Dup => {
                    let value = *try!(self.stack
                                          .last()
                                          .ok_or(HintingExecutionError::StackUnderflow));
                    self.stack.push(value);
                }
                Instruction::Pop => {
                    try!(self.pop());
                }
                Instruction::Clear => self.stack.clear(),
                Instruction::Swap => {
                    let (a, b) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push(a);
                    self.stack.push(b);
                }
                Instruction::Mindex => {
                    let index = try!(self.pop()) as usize;
                    if index >= self.stack.len() {
                        return Err(HintingExecutionError::StackUnderflow)
                    }
                    let rindex = self.stack.len() - 1 - index;
                    let value = self.stack.remove(rindex);
                    self.stack.push(value)
                }
                Instruction::If => {
                    let cond = try!(self.pop());
                    if cond == 0 {
                        // Move to the instruction following `else` or `eif`.
                        let else_target_index = self.scripts[frame.script]
                                                    .branch_targets
                                                    .binary_search_by(|script| {
                                                        script.branch_location.cmp(&frame.pc)
                                                    }).unwrap();
                        new_pc = self.scripts[frame.script]
                                     .branch_targets[else_target_index]
                                     .target_location + 1
                    }
                }
                Instruction::Else => {
                    // The only way we get here is by falling off the end of a then-branch. So jump
                    // to the instruction following the matching `eif`.
                    let eif_target_index = self.scripts[frame.script]
                                               .branch_targets
                                               .binary_search_by(|script| {
                                                script.branch_location.cmp(&frame.pc)
                                               }).unwrap();
                    new_pc = self.scripts[frame.script]
                                 .branch_targets[eif_target_index]
                                 .target_location + 1
                }
                Instruction::Eif => {
                    // Likewise, the only way we get here is by falling off the end of a
                    // then-branch.
                }
                Instruction::Lt => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs < rhs) as i32)
                }
                Instruction::Lteq => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs <= rhs) as i32)
                }
                Instruction::Gt => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs > rhs) as i32)
                }
                Instruction::Gteq => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs >= rhs) as i32)
                }
                Instruction::Eq => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs == rhs) as i32)
                }
                Instruction::Neq => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs != rhs) as i32)
                }
                Instruction::And => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs != 0 && rhs != 0) as i32)
                }
                Instruction::Or => {
                    let (rhs, lhs) = (try!(self.pop()), try!(self.pop()));
                    self.stack.push((lhs != 0 || rhs != 0) as i32)
                }
                Instruction::Not => {
                    let cond = try!(self.pop());
                    self.stack.push((cond == 0) as i32)
                }
                Instruction::Add => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    self.stack.push((lhs + rhs).0)
                }
                Instruction::Sub => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    self.stack.push((lhs - rhs).0)
                }
                Instruction::Div => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    if rhs.is_zero() {
                        // Obey Postel's law…
                        self.stack.push(F26DOT6_ZERO.0)
                    } else {
                        self.stack.push((lhs / rhs).0)
                    }
                }
                Instruction::Mul => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    self.stack.push((lhs * rhs).0)
                }
                Instruction::Abs => {
                    // Actually in fixed point, but it works out the same way.
                    let n = try!(self.pop());
                    self.stack.push(n.abs())
                }
                Instruction::Neg => {
                    let n = F26Dot6(try!(self.pop()));
                    self.stack.push((-n).0)
                }
                Instruction::Max => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    self.stack.push(cmp::max(rhs, lhs).0)
                }
                Instruction::Min => {
                    let (rhs, lhs) = (F26Dot6(try!(self.pop())), F26Dot6(try!(self.pop())));
                    self.stack.push(cmp::max(rhs, lhs).0)
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
                Instruction::Call => {
                    let id = try!(self.pop()) as usize;
                    let new_frame = match self.functions.get(id) {
                        Some(&Some(new_frame)) => new_frame,
                        Some(&None) | None => {
                            return Err(HintingExecutionError::CallToUndefinedFunction)
                        }
                    };

                    // Save our return address.
                    self.call_stack.last_mut().unwrap().pc = new_pc;

                    // Jump to the new function.
                    self.call_stack.push(new_frame);
                    new_pc = new_frame.pc
                }
                Instruction::Getinfo => {
                    let selector = InfoSelector::from_bits_truncate(try!(self.pop()));

                    // We only handle a subset of the selectors here.
                    //
                    // TODO(pcwalton): Handle the ones relating to subpixel AA.
                    let mut result = 0;
                    if selector.contains(VERSION) {
                        result |= GETINFO_VERSION
                    }
                    if selector.contains(FONT_SMOOTHING_GRAYSCALE) {
                        result |= 1 << INFO_RESULT_FONT_SMOOTHING_GRAYSCALE_SHIFT
                    }
                    self.stack.push(result)
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

