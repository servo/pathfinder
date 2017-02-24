// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(dead_code)]

//! The TrueType hinting VM.
//!
//! See: https://www.microsoft.com/typography/otspec/ttinst.htm

use byteorder::{BigEndian, ByteOrder};
use error::HintingError;
use euclid::Point2D;
use font::Font;

mod insns;

/// A TrueType hinting virtual machine.
pub struct Hinter {
    // The Control Value Table: the VM's initialized memory.
    control_value_table: Vec<i16>,
    // The Storage Area: the VM's uninitialized memory.
    storage_area: Vec<u32>,
    // The projection vector, in 2.14 fixed point.
    projection_vector: Point2D<i16>,
    // The dual projection vector, in 2.14 fixed point.
    dual_projection_vector: Point2D<i16>,
    // The freedom vector, in 2.14 fixed point.
    freedom_vector: Point2D<i16>,
    // The reference point indices.
    reference_points: [u32; 3],
    // The zone numbers.
    zone_points: [u32; 3],
    // The round state.
    round_state: RoundState,
    // The loop variable count.
    loop_count: u32,
    // The minimum distance value.
    minimum_distance: u32,
    // Instruction control flags.
    instruction_control: InstructionControl,
    // Threshold value for ppem. See `SCANCTRL` (ttinst1.doc, 244-245).
    dropout_threshold: u8,
    // Special dropout control.
    dropout_control: DropoutControl,
    // The scan type. See `SCANTYPE` (ttinst1.doc, 246-247).
    scan_type: ScanType,
    // The control value cut in. See `SCVTSI` (ttinst1.doc, 249).
    control_value_cut_in: u32,
    // The single width cut in. See `SSWCI` (ttinst1.doc, 250).
    single_width_cut_in: u32,
    // The single width value. See `SSW` (ttinst1.doc, 251).
    single_width_value: i32,
    // The angle weight. Per spec, does nothing. See `SANGW` (ttinst1.doc, 254).
    angle_weight: u32,
    // The delta base. See `SDB` (ttinst1.doc, 255).
    delta_base: u32,
    // The delta shift. See `SDS` (ttinst1.doc, 256).
    delta_shift: u32,
    // Various graphics state flags.
    graphics_state_flags: GraphicsStateFlags,
}

impl Hinter {
    pub fn new(font: &Font) -> Result<Hinter, HintingError> {
        let cvt = font.control_value_table().chunks(2).map(BigEndian::read_i16).collect();
        let hinter = Hinter {
            control_value_table: cvt,
            storage_area: vec![],
            projection_vector: Point2D::zero(),
            dual_projection_vector: Point2D::zero(),
            freedom_vector: Point2D::zero(),
            reference_points: [0; 3],
            zone_points: [0; 3],
            round_state: RoundState::RoundToHalfGrid,
            loop_count: 0,
            minimum_distance: 0,
            instruction_control: InstructionControl::empty(),
            dropout_threshold: 0,
            dropout_control: DropoutControl::empty(),
            scan_type: ScanType::SimpleDropoutControlIncludingStubs,
            control_value_cut_in: 0,
            single_width_cut_in: 0,
            single_width_value: 0,
            angle_weight: 0,
            delta_base: 0,
            delta_shift: 0,
            graphics_state_flags: AUTO_FLIP,
        };

        Ok(hinter)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
enum RoundState {
    RoundToHalfGrid = 0,
    RoundToGrid = 1,
    RoundToDoubleGrid = 2,
    RoundDownToGrid = 3,
    RoundUpToGrid = 4,
    RoundOff = 5,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum ScanType {
    SimpleDropoutControlIncludingStubs = 0,
    SimpleDropoutControlExcludingStubs = 1,
    NoDropoutControl = 2,
    SmartDropoutControlIncludingStubs = 3,
    SmartDropoutControlExcludingStubs = 4,
}

bitflags! {
    pub flags InstructionControl: u8 {
        const INHIBIT_GRID_FITTING = 1 << 0,
        const IGNORE_CVT_PARAMETERS = 1 << 1,
        const NATIVE_SUBPIXEL_AA = 1 << 2,
    }
}

bitflags! {
    pub flags DropoutControl: u8 {
        const DROPOUT_IF_PPEM_LESS_THAN_THRESHOLD = 1 << 0,
        const DROPOUT_IF_ROTATED = 1 << 1,
        const DROPOUT_IF_STRETCHED = 1 << 2,
        const NO_DROPOUT_IF_PPEM_GREATER_THAN_THRESHOLD = 1 << 3,
        const NO_DROPOUT_IF_UNROTATED = 1 << 4,
        const NO_DROPOUT_IF_UNSTRETCHED = 1 << 5,
    }
}

bitflags! {
    flags GraphicsStateFlags: u8 {
        // See `FLIPON` (default true) (ttinst1.doc, 252).
        const AUTO_FLIP = 1 << 1,
    }
}

