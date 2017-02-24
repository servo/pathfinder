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

// All TrueType instructions.
#[derive(Clone, Copy, Debug)]
enum Instruction<'a> {
    // Push N Bytes (0x40) (ttinst1.doc, p. 189)
    Npushb(&'a [u8]),
    // Push N Words (0x41) (ttinst1.doc, p. 190)
    Npushw(&'a [u8]),
    // Push Bytes (0xb0-0xb7) (ttinst1.doc, p. 191)
    Pushb(u8, &'a [u8]),
    // Push Words (0xb8-0xbf) (ttinst1.doc, p. 192)
    Pushw(u8, &'a [u8]),
    // Read Store (0x43) (ttinst1.doc, p. 194)
    Rs,
    // Write Store (0x42) (ttinst1.doc, p. 195)
    Ws,
    // Write Control Value Table in Pixel Units (0x44) (ttinst1.doc, 197)
    Wcvtp,
    // Write Control Value Table in FUints (0x70) (ttinst1.doc, 198)
    Wcvtf,
    // Read Control Value Table (0x45) (ttinst1.doc, 199)
    Rcvt,
    // Set Freedom and Projection Vectors to Coordinate Axis (0x00-0x01) (ttinst1.doc, 202)
    Svtca(Axis),
    // Set Projection Vector to Coordinate Axis (0x02-0x03) (ttinst1.doc, 203)
    Spvtca(Axis),
    // Set Freedom Vector to Coordinate Axis (0x04-0x05) (ttinst1.doc, 204)
    Sfvtca(Axis),
    // Set Projection Vector to Line (0x06-0x07) (ttinst1.doc, 205-207)
    Spvtl(LineOrientation),
    // Set Freedom Vector to Line (0x08-0x09) (ttinst1.doc, 208-209)
    Sfvtl(LineOrientation),
    // Set Freedom Vector to Projection Vector (0x0e) (ttinst1.doc, 210)
    Sfvtpv,
    // Set Dual Projection Vector to Line (0x86-0x87) (ttinst1.doc, 211)
    Sdpvtl(LineOrientation),
    // Set Projection Vector From Stack (0x0a) (ttinst1.doc, 212-213)
    Spvfs,
    // Set Freedom Vector From Stack (0x0b) (ttinst1.doc, 214-215)
    Sfvfs,
    // Get Projection Vector (0x0c) (ttinst1.doc, 216-217)
    Gpv,
    // Get Freedom Vector (0x0d) (ttinst1.doc, 218-219)
    Gfv,
    // Set Reference Point 0 (0x10) (ttinst1.doc, 220)
    Srp0,
    // Set Reference Point 1 (0x11) (ttinst1.doc, 221)
    Srp1,
    // Set Reference Point 2 (0x12) (ttinst1.doc, 222)
    Srp2,
    // Set Zone Pointer 0 (0x13) (ttinst1.doc, 223)
    Szp0,
    // Set Zone Pointer 1 (0x14) (ttinst1.doc, 224)
    Szp1,
    // Set Zone Pointer 2 (0x15) (ttinst1.doc, 225)
    Szp2,
    // Set Zone Pointers (0x16) (ttinst1.doc, 226)
    Szps,
    // Round to Half Grid (0x19) (ttinst1.doc, 227)
    Rthg,
    // Round to Grid (0x18) (ttinst1.doc, 228)
    Rtg,
    // Round to Double Grid (0x3d) (ttinst1.doc, 229)
    Rtdg,
    // Round Down to Grid (0x7d) (ttinst1.doc, 230)
    Rdtg,
    // Round Up to Grid (0x7c) (ttinst1.doc, 231)
    Rutg,
    // Round Off (0x7a) (ttinst1.doc, 232)
    Roff,
    // Super Round (0x76) (ttinst1.doc, 233-238)
    Sround,
    // Super Round 45 Degrees (0x77) (ttinst1.doc, 239)
    S45round,
    // Set Loop Variable (0x17) (ttinst1.doc, 240)
    Sloop,
    // Set Minimum Distance (0x1a) (ttinst1.doc, 241)
    Smd,
    // Instruction Execution Control (0x8e) (ttinst1.doc, 242-243)
    Instctrl,
    // Scan Conversion Control (0x85) (ttinst1.doc, 244-245)
    Scanctrl,
    // Set Control Value Table Cut In (0x1d) (ttinst1.doc, 249)
    Scvtci,
    // Set Single Width Cut In (0x1e) (ttinst1.doc, 250)
    Sswci,
    // Set Single Width (0x1f) (ttinst1.doc, 251)
    Ssw,
    // Set Auto Flip Boolean to On (0x4d) (ttinst1.doc, 252)
    Flipon,
    // Set Auto Flip Boolean to Off (0x4e) (ttinst1.doc, 253)
    Flipoff,
    // Set Angle Weight (0x7e) (ttinst1.doc, 254)
    Sangw,
    // Set Delta Base (0x5e) (ttinst1.doc, 255)
    Sdb,
    // Set Delta Shift (0x5f) (ttinst1.doc, 256)
    Sds,
    // Get Coordinate (0x46-0x47) (ttinst1.doc, 258-259)
    Gc(WhichPosition),
    // Set Coordinate From Stack (0x48) (ttinst1.doc, 260)
    Scfs,
    // Measure Distance (0x49-0x4a) (ttinst1.doc, 261-262)
    Md(WhichPosition),
    // Measure Pixels Per Em (0x4b) (ttinst1.doc, 263)
    Mppem,
    // Measure Point Size (0x4c) (ttinst1.doc, 264)
    Mps,
    // Flip Point (0x80) (ttinst2.doc, 263)
    Flippt,
    // Flip Range On (0x81) (ttinst2.doc, 264)
    Fliprgon,
    // Flip Range Off (0x82) (ttinst2.doc, 265)
    Fliprgoff,
    // Shift Point by Last Point (0x32-0x33) (ttinst2.doc, 266)
    Shp(ZonePoint),
    // Shift Contour by Last Point (0x34-0x35) (ttinst2.doc, 267)
    Shc(ZonePoint),
    // Shift Zone by Last Point (0x36-0x37) (ttinst2.doc, 268)
    Shz(ZonePoint),
    // Shift Point by Pixel Amount (0x38) (ttinst2.doc, 269)
    Shpix,
    // Move Stack Indirect Relative Point (0x3a-0x3b) (ttinst2.doc, 270)
    Msirp(SetRP0),
    // Move Direct Absolute Point (0x2e-0x2f) (ttinst2.doc, 271)
    Mdap(ShouldRound),
    // Move Indirect Absolute Point (0x3e-0x3f) (ttinst2.doc, 272-275)
    Miap(ShouldRound),
    // Move Direct Relative Point (0xc0-0xdf) (ttinst2.doc, 276-283)
    Mdrp(SetRP0, ApplyMinimumDistance, ShouldRound, DistanceType),
    // Align Relative Point (0x3c) (ttinst2.doc, 284)
    Alignrp,
    // Move Point to Intersection of Two Lines (0x0f) (ttinst2.doc, 286-288)
    Isect,
    // Align Points (0x27) (ttinst2.doc, 289)
    Alignpts,
    // Interpolate Point by Last Relative Stretch (0x39) (ttinst2.doc, 290)
    Ip,
    // Untouch Point (0x29) (ttinst2.doc, 291)
    Utp,
    // Interpolate Untouched Points Through Outline (0x30-0x31) (ttinst2.doc, 292)
    Iup(Axis),
    // Delta Exception P1 (0x5d) (ttinst2.doc, 296)
    Deltap1,
    // Delta Exception P2 (0x71) (ttinst2.doc, 297)
    Deltap2,
    // Delta Exception P3 (0x72) (ttinst2.doc, 298)
    Deltap3,
    // Delta Exception C1 (0x73) (ttinst2.doc, 299)
    Deltac1,
    // Delta Exception C2 (0x74) (ttinst2.doc, 300)
    Deltac2,
    // Delta Exception C3 (0x75) (ttinst2.doc, 301)
    Deltac3,
    // Duplicate Top Stack Element (0x20) (ttinst2.doc, 304)
    Dup,
    // Pop Top Stack Element (0x21) (ttinst2.doc, 305)
    Pop,
    // Clear the Entire Stack (0x22) (ttinst2.doc, 306)
    Clear,
    // Swap the Top Two Elements on the Stack (0x23) (ttinst2.doc, 307)
    Swap,
    // Return the Depth of the Stack (0x24) (ttinst2.doc, 308)
    Depth,
    // Copy an Indexed Element to the Top of the Stack (0x25) (ttinst2.doc, 309)
    Cindex,
    // Move an Indexed Element to the Top of the Stack (0x26) (ttinst2.doc, 310)
    Mindex,
    // Roll the Top Three Stack Elements (0x8a) (ttinst2.doc, 311)
    Roll,
    // If Test (0x58) (ttinst2.doc, 313-314)
    If,
    // Else (0x1b) (ttinst2.doc, 315)
    Else,
    // End If (0x59) (ttinst2.doc, 316)
    EIf,
    // Jump Relative on True (0x78) (ttinst2.doc, 317-318)
    Jrot,
    // Jump (0x1c) (ttinst2.doc, 319)
    Jmpr,
    // Jump Relative on False (0x79) (ttinst2.doc, 320-321)
    Jrof,
    // Less Than (0x50) (ttinst2.doc, 323)
    Lt,
    // Less Than or Equal (0x51) (ttinst2.doc, 324)
    Lteq,
    // Greater Than (0x52) (ttinst2.doc, 325)
    Gt,
    // Greater Than or Equal (0x53) (ttinst2.doc, 326)
    Gteq,
    // Equal (0x54) (ttinst2.doc, 327)
    Eq,
    // Not Equal (0x55) (ttinst2.doc, 328)
    Neq,
    // Odd (0x56) (ttinst2.doc, 329)
    Odd,
    // Even (0x57) (ttinst2.doc, 330)
    Even,
    // Logical And (0x5a) (ttinst2.doc, 331-332)
    And,
    // Logical Or (0x5b) (ttinst2.doc, 333)
    Or,
    // Logical Not (0x5c) (ttinst2.doc, 334)
    Not,
    // Add (0x60) (ttinst2.doc, 336)
    Add,
    // Subtract (0x61) (ttinst2.doc, 337)
    Sub,
    // Divide (0x62) (ttinst2.doc, 338)
    Div,
    // Multiply (0x63) (ttinst2.doc, 339)
    Mul,
    // Absolute Value (0x64) (ttinst2.doc, 340)
    Abs,
    // Negate (0x65) (ttinst2.doc, 341)
    Neg,
    // Floor (0x66) (ttinst2.doc, 342)
    Floor,
    // Ceiling (0x67) (ttinst2.doc, 343)
    Ceiling,
    // Maximum of Top Two Stack Elements (0x8b) (ttinst2.doc, 344)
    Max,
    // Minimum of Top Two Stack Elements (0x8c) (ttinst2.doc, 345)
    Min,
    // Round Value (0x68-0x6b) (ttinst2.doc, 347)
    Round(DistanceType),
    // No Rounding of Value (0x6c-0x6f) (ttinst2.doc, 349)
    Nround(DistanceType),
    // Function Definition (0x2c) (ttinst2.doc, 351)
    Fdef,
    // End Function Definition (0x2d) (ttinst2.doc, 352)
    Endf,
    // Call Function (0x2b) (ttinst2.doc, 353)
    Call,
    // Loop and Call Function (0x2a) (ttinst2.doc, 354)
    Loopcall,
    // Instruction Definition (0x89) (ttinst2.doc, 355)
    Idef,
    // Debug Call (0x4f) (ttinst2.doc, 356)
    Debug,
    // Get Information (0x88) (ttinst2.doc, 357-360)
    Getinfo,
    // Get Variation (0x91) (ttinst2.doc, 361)
    Getvariation,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
enum Axis {
    Y = 0,
    X = 1,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
enum LineOrientation {
    Parallel = 0,
    Perpendicular = 1,
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

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
enum WhichPosition {
    // Use the current position.
    Current = 0,
    // Use the position in the original outline.
    Original = 1,
}

bitflags! {
    flags InstructionControl: u8 {
        const INHIBIT_GRID_FITTING = 1 << 0,
        const IGNORE_CVT_PARAMETERS = 1 << 1,
        const NATIVE_SUBPIXEL_AA = 1 << 2,
    }
}

bitflags! {
    flags DropoutControl: u8 {
        const DROPOUT_IF_PPEM_LESS_THAN_THRESHOLD = 1 << 0,
        const DROPOUT_IF_ROTATED = 1 << 1,
        const DROPOUT_IF_STRETCHED = 1 << 2,
        const NO_DROPOUT_IF_PPEM_GREATER_THAN_THRESHOLD = 1 << 3,
        const NO_DROPOUT_IF_UNROTATED = 1 << 4,
        const NO_DROPOUT_IF_UNSTRETCHED = 1 << 5,
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
enum ScanType {
    SimpleDropoutControlIncludingStubs = 0,
    SimpleDropoutControlExcludingStubs = 1,
    NoDropoutControl = 2,
    SmartDropoutControlIncludingStubs = 3,
    SmartDropoutControlExcludingStubs = 4,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
enum ZonePoint {
    Zone1Point2 = 0,
    Zone0Point1 = 1,
}

#[derive(Copy, Clone, PartialEq, Debug)]
struct SetRP0(pub bool);

#[derive(Copy, Clone, PartialEq, Debug)]
struct ApplyMinimumDistance(pub bool);

#[derive(Copy, Clone, PartialEq, Debug)]
struct ShouldRound(pub bool);

// See `MDRP` (ttinst2.doc, 277)
#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
enum DistanceType {
    Gray = 0,
    Black = 1,
    White = 2,
}

bitflags! {
    flags GraphicsStateFlags: u8 {
        // See `FLIPON` (default true) (ttinst1.doc, 252).
        const AUTO_FLIP = 1 << 1,
    }
}

