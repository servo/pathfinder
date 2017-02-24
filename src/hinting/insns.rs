// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! TrueType instructions.

/// All TrueType instructions.
#[derive(Clone, Copy, Debug)]
pub enum Instruction<'a> {
    /// Push N Bytes (0x40) (ttinst1.doc, p. 189)
    /// Push Bytes (0xb0-0xb7) (ttinst1.doc, p. 191)
    Pushb(&'a [u8]),
    /// Push N Words (0x41) (ttinst1.doc, p. 190)
    /// Push Words (0xb8-0xbf) (ttinst1.doc, p. 192)
    Pushw(&'a [u8]),
    /// Read Store (0x43) (ttinst1.doc, p. 194)
    Rs,
    /// Write Store (0x42) (ttinst1.doc, p. 195)
    Ws,
    /// Write Control Value Table in Pixel Units (0x44) (ttinst1.doc, 197)
    Wcvtp,
    /// Write Control Value Table in FUints (0x70) (ttinst1.doc, 198)
    Wcvtf,
    /// Read Control Value Table (0x45) (ttinst1.doc, 199)
    Rcvt,
    /// Set Freedom and Projection Vectors to Coordinate Axis (0x00-0x01) (ttinst1.doc, 202)
    Svtca(Axis),
    /// Set Projection Vector to Coordinate Axis (0x02-0x03) (ttinst1.doc, 203)
    Spvtca(Axis),
    /// Set Freedom Vector to Coordinate Axis (0x04-0x05) (ttinst1.doc, 204)
    Sfvtca(Axis),
    /// Set Projection Vector to Line (0x06-0x07) (ttinst1.doc, 205-207)
    Spvtl(LineOrientation),
    /// Set Freedom Vector to Line (0x08-0x09) (ttinst1.doc, 208-209)
    Sfvtl(LineOrientation),
    /// Set Freedom Vector to Projection Vector (0x0e) (ttinst1.doc, 210)
    Sfvtpv,
    /// Set Dual Projection Vector to Line (0x86-0x87) (ttinst1.doc, 211)
    Sdpvtl(LineOrientation),
    /// Set Projection Vector From Stack (0x0a) (ttinst1.doc, 212-213)
    Spvfs,
    /// Set Freedom Vector From Stack (0x0b) (ttinst1.doc, 214-215)
    Sfvfs,
    /// Get Projection Vector (0x0c) (ttinst1.doc, 216-217)
    Gpv,
    /// Get Freedom Vector (0x0d) (ttinst1.doc, 218-219)
    Gfv,
    /// Set Reference Point 0 (0x10) (ttinst1.doc, 220)
    Srp0,
    /// Set Reference Point 1 (0x11) (ttinst1.doc, 221)
    Srp1,
    /// Set Reference Point 2 (0x12) (ttinst1.doc, 222)
    Srp2,
    /// Set Zone Pointer 0 (0x13) (ttinst1.doc, 223)
    Szp0,
    /// Set Zone Pointer 1 (0x14) (ttinst1.doc, 224)
    Szp1,
    /// Set Zone Pointer 2 (0x15) (ttinst1.doc, 225)
    Szp2,
    /// Set Zone Pointers (0x16) (ttinst1.doc, 226)
    Szps,
    /// Round to Half Grid (0x19) (ttinst1.doc, 227)
    Rthg,
    /// Round to Grid (0x18) (ttinst1.doc, 228)
    Rtg,
    /// Round to Double Grid (0x3d) (ttinst1.doc, 229)
    Rtdg,
    /// Round Down to Grid (0x7d) (ttinst1.doc, 230)
    Rdtg,
    /// Round Up to Grid (0x7c) (ttinst1.doc, 231)
    Rutg,
    /// Round Off (0x7a) (ttinst1.doc, 232)
    Roff,
    /// Super Round (0x76) (ttinst1.doc, 233-238)
    Sround,
    /// Super Round 45 Degrees (0x77) (ttinst1.doc, 239)
    S45round,
    /// Set Loop Variable (0x17) (ttinst1.doc, 240)
    Sloop,
    /// Set Minimum Distance (0x1a) (ttinst1.doc, 241)
    Smd,
    /// Instruction Execution Control (0x8e) (ttinst1.doc, 242-243)
    Instctrl,
    /// Scan Conversion Control (0x85) (ttinst1.doc, 244-245)
    Scanctrl,
    /// Set Control Value Table Cut In (0x1d) (ttinst1.doc, 249)
    Scvtci,
    /// Set Single Width Cut In (0x1e) (ttinst1.doc, 250)
    Sswci,
    /// Set Single Width (0x1f) (ttinst1.doc, 251)
    Ssw,
    /// Set Auto Flip Boolean to On (0x4d) (ttinst1.doc, 252)
    Flipon,
    /// Set Auto Flip Boolean to Off (0x4e) (ttinst1.doc, 253)
    Flipoff,
    /// Set Angle Weight (0x7e) (ttinst1.doc, 254)
    Sangw,
    /// Set Delta Base (0x5e) (ttinst1.doc, 255)
    Sdb,
    /// Set Delta Shift (0x5f) (ttinst1.doc, 256)
    Sds,
    /// Get Coordinate (0x46-0x47) (ttinst1.doc, 258-259)
    Gc(WhichPosition),
    /// Set Coordinate From Stack (0x48) (ttinst1.doc, 260)
    Scfs,
    /// Measure Distance (0x49-0x4a) (ttinst1.doc, 261-262)
    Md(WhichPosition),
    /// Measure Pixels Per Em (0x4b) (ttinst1.doc, 263)
    Mppem,
    /// Measure Point Size (0x4c) (ttinst1.doc, 264)
    Mps,
    /// Flip Point (0x80) (ttinst2.doc, 263)
    Flippt,
    /// Flip Range On (0x81) (ttinst2.doc, 264)
    Fliprgon,
    /// Flip Range Off (0x82) (ttinst2.doc, 265)
    Fliprgoff,
    /// Shift Point by Last Point (0x32-0x33) (ttinst2.doc, 266)
    Shp(ZonePoint),
    /// Shift Contour by Last Point (0x34-0x35) (ttinst2.doc, 267)
    Shc(ZonePoint),
    /// Shift Zone by Last Point (0x36-0x37) (ttinst2.doc, 268)
    Shz(ZonePoint),
    /// Shift Point by Pixel Amount (0x38) (ttinst2.doc, 269)
    Shpix,
    /// Move Stack Indirect Relative Point (0x3a-0x3b) (ttinst2.doc, 270)
    Msirp(SetRP0),
    /// Move Direct Absolute Point (0x2e-0x2f) (ttinst2.doc, 271)
    Mdap(ShouldRound),
    /// Move Indirect Absolute Point (0x3e-0x3f) (ttinst2.doc, 272-275)
    Miap(ShouldRound),
    /// Move Direct Relative Point (0xc0-0xdf) (ttinst2.doc, 276-283)
    Mdrp(SetRP0, ApplyMinimumDistance, ShouldRound, DistanceType),
    /// Align Relative Point (0x3c) (ttinst2.doc, 284)
    Alignrp,
    /// Move Point to Intersection of Two Lines (0x0f) (ttinst2.doc, 286-288)
    Isect,
    /// Align Points (0x27) (ttinst2.doc, 289)
    Alignpts,
    /// Interpolate Point by Last Relative Stretch (0x39) (ttinst2.doc, 290)
    Ip,
    /// Untouch Point (0x29) (ttinst2.doc, 291)
    Utp,
    /// Interpolate Untouched Points Through Outline (0x30-0x31) (ttinst2.doc, 292)
    Iup(Axis),
    /// Delta Exception P1 (0x5d) (ttinst2.doc, 296)
    Deltap1,
    /// Delta Exception P2 (0x71) (ttinst2.doc, 297)
    Deltap2,
    /// Delta Exception P3 (0x72) (ttinst2.doc, 298)
    Deltap3,
    /// Delta Exception C1 (0x73) (ttinst2.doc, 299)
    Deltac1,
    /// Delta Exception C2 (0x74) (ttinst2.doc, 300)
    Deltac2,
    /// Delta Exception C3 (0x75) (ttinst2.doc, 301)
    Deltac3,
    /// Duplicate Top Stack Element (0x20) (ttinst2.doc, 304)
    Dup,
    /// Pop Top Stack Element (0x21) (ttinst2.doc, 305)
    Pop,
    /// Clear the Entire Stack (0x22) (ttinst2.doc, 306)
    Clear,
    /// Swap the Top Two Elements on the Stack (0x23) (ttinst2.doc, 307)
    Swap,
    /// Return the Depth of the Stack (0x24) (ttinst2.doc, 308)
    Depth,
    /// Copy an Indexed Element to the Top of the Stack (0x25) (ttinst2.doc, 309)
    Cindex,
    /// Move an Indexed Element to the Top of the Stack (0x26) (ttinst2.doc, 310)
    Mindex,
    /// Roll the Top Three Stack Elements (0x8a) (ttinst2.doc, 311)
    Roll,
    /// If Test (0x58) (ttinst2.doc, 313-314)
    If,
    /// Else (0x1b) (ttinst2.doc, 315)
    Else,
    /// End If (0x59) (ttinst2.doc, 316)
    EIf,
    /// Jump Relative on True (0x78) (ttinst2.doc, 317-318)
    Jrot,
    /// Jump (0x1c) (ttinst2.doc, 319)
    Jmpr,
    /// Jump Relative on False (0x79) (ttinst2.doc, 320-321)
    Jrof,
    /// Less Than (0x50) (ttinst2.doc, 323)
    Lt,
    /// Less Than or Equal (0x51) (ttinst2.doc, 324)
    Lteq,
    /// Greater Than (0x52) (ttinst2.doc, 325)
    Gt,
    /// Greater Than or Equal (0x53) (ttinst2.doc, 326)
    Gteq,
    /// Equal (0x54) (ttinst2.doc, 327)
    Eq,
    /// Not Equal (0x55) (ttinst2.doc, 328)
    Neq,
    /// Odd (0x56) (ttinst2.doc, 329)
    Odd,
    /// Even (0x57) (ttinst2.doc, 330)
    Even,
    /// Logical And (0x5a) (ttinst2.doc, 331-332)
    And,
    /// Logical Or (0x5b) (ttinst2.doc, 333)
    Or,
    /// Logical Not (0x5c) (ttinst2.doc, 334)
    Not,
    /// Add (0x60) (ttinst2.doc, 336)
    Add,
    /// Subtract (0x61) (ttinst2.doc, 337)
    Sub,
    /// Divide (0x62) (ttinst2.doc, 338)
    Div,
    /// Multiply (0x63) (ttinst2.doc, 339)
    Mul,
    /// Absolute Value (0x64) (ttinst2.doc, 340)
    Abs,
    /// Negate (0x65) (ttinst2.doc, 341)
    Neg,
    /// Floor (0x66) (ttinst2.doc, 342)
    Floor,
    /// Ceiling (0x67) (ttinst2.doc, 343)
    Ceiling,
    /// Maximum of Top Two Stack Elements (0x8b) (ttinst2.doc, 344)
    Max,
    /// Minimum of Top Two Stack Elements (0x8c) (ttinst2.doc, 345)
    Min,
    /// Round Value (0x68-0x6b) (ttinst2.doc, 347)
    Round(DistanceType),
    /// No Rounding of Value (0x6c-0x6f) (ttinst2.doc, 349)
    Nround(DistanceType),
    /// Function Definition (0x2c) (ttinst2.doc, 351)
    Fdef,
    /// End Function Definition (0x2d) (ttinst2.doc, 352)
    Endf,
    /// Call Function (0x2b) (ttinst2.doc, 353)
    Call,
    /// Loop and Call Function (0x2a) (ttinst2.doc, 354)
    Loopcall,
    /// Instruction Definition (0x89) (ttinst2.doc, 355)
    Idef,
    /// Debug Call (0x4f) (ttinst2.doc, 356)
    Debug,
    /// Get Information (0x88) (ttinst2.doc, 357-360)
    Getinfo,
    /// Get Variation (0x91) (ttinst2.doc, 361)
    Getvariation,
}

impl<'a> Instruction<'a> {
    #[inline]
    pub fn parse<'b, 'c>(data: &'b [u8], pc: &'c mut usize)
                         -> Result<Instruction<'b>, ParseError> {
        let op = try!(get(data, pc).ok_or(ParseError::Eof));
        match op {
            0x40 | 0xb0...0xb7 => {
                let count = if op == 0x40 {
                    try!(get(data, pc).ok_or(ParseError::UnexpectedEof)) as usize
                } else {
                    (op as usize & 7) + 1
                };
                if *pc + count <= data.len() {
                    let insn = Instruction::Pushb(&data[*pc..(*pc + count)]);
                    *pc += count;
                    Ok(insn)
                } else {
                    Err(ParseError::UnexpectedEof)
                }
            }
            0x41 | 0xb8...0xbf => {
                let count = if op == 0x41 {
                    try!(get(data, pc).ok_or(ParseError::UnexpectedEof)) as usize * 2
                } else {
                    ((op as usize & 7) + 1) * 2
                };
                if *pc + count <= data.len() {
                    let insn = Instruction::Pushw(&data[*pc..(*pc + count)]);
                    *pc += count;
                    Ok(insn)
                } else {
                    Err(ParseError::UnexpectedEof)
                }
            }
            0x43 => Ok(Instruction::Rs),
            0x42 => Ok(Instruction::Ws),
            0x44 => Ok(Instruction::Wcvtp),
            0x70 => Ok(Instruction::Wcvtf),
            0x45 => Ok(Instruction::Rcvt),
            0x00 => Ok(Instruction::Svtca(Axis::Y)),
            0x01 => Ok(Instruction::Svtca(Axis::X)),
            0x02 => Ok(Instruction::Spvtca(Axis::Y)),
            0x03 => Ok(Instruction::Spvtca(Axis::X)),
            0x04 => Ok(Instruction::Sfvtca(Axis::Y)),
            0x05 => Ok(Instruction::Sfvtca(Axis::X)),
            0x06 => Ok(Instruction::Spvtl(LineOrientation::Parallel)),
            0x07 => Ok(Instruction::Spvtl(LineOrientation::Perpendicular)),
            0x08 => Ok(Instruction::Sfvtl(LineOrientation::Parallel)),
            0x09 => Ok(Instruction::Sfvtl(LineOrientation::Perpendicular)),
            0x0e => Ok(Instruction::Sfvtpv),
            0x86 => Ok(Instruction::Sdpvtl(LineOrientation::Parallel)),
            0x87 => Ok(Instruction::Sdpvtl(LineOrientation::Perpendicular)),
            0x0a => Ok(Instruction::Spvfs),
            0x0b => Ok(Instruction::Sfvfs),
            0x0c => Ok(Instruction::Gpv),
            0x0d => Ok(Instruction::Gfv),
            0x10 => Ok(Instruction::Srp0),
            0x11 => Ok(Instruction::Srp1),
            0x12 => Ok(Instruction::Srp2),
            0x13 => Ok(Instruction::Szp0),
            0x14 => Ok(Instruction::Szp1),
            0x15 => Ok(Instruction::Szp2),
            0x16 => Ok(Instruction::Szps),
            0x19 => Ok(Instruction::Rthg),
            0x18 => Ok(Instruction::Rtg),
            0x3d => Ok(Instruction::Rtdg),
            0x7d => Ok(Instruction::Rdtg),
            0x7c => Ok(Instruction::Rutg),
            0x7a => Ok(Instruction::Roff),
            0x76 => Ok(Instruction::Sround),
            0x77 => Ok(Instruction::S45round),
            0x17 => Ok(Instruction::Sloop),
            0x1a => Ok(Instruction::Smd),
            0x8e => Ok(Instruction::Instctrl),
            0x85 => Ok(Instruction::Scanctrl),
            0x1d => Ok(Instruction::Scvtci),
            0x1e => Ok(Instruction::Sswci),
            0x1f => Ok(Instruction::Ssw),
            0x4d => Ok(Instruction::Flipon),
            0x4e => Ok(Instruction::Flipoff),
            0x7e => Ok(Instruction::Sangw),
            0x5e => Ok(Instruction::Sdb),
            0x5f => Ok(Instruction::Sds),
            0x46 => Ok(Instruction::Gc(WhichPosition::Current)),
            0x47 => Ok(Instruction::Gc(WhichPosition::Original)),
            0x48 => Ok(Instruction::Scfs),
            0x49 => Ok(Instruction::Md(WhichPosition::Current)),
            0x4a => Ok(Instruction::Md(WhichPosition::Original)),
            0x4b => Ok(Instruction::Mppem),
            0x4c => Ok(Instruction::Mps),
            0x80 => Ok(Instruction::Flippt),
            0x81 => Ok(Instruction::Fliprgon),
            0x82 => Ok(Instruction::Fliprgoff),
            0x32 => Ok(Instruction::Shp(ZonePoint::Zone1Point2)),
            0x33 => Ok(Instruction::Shp(ZonePoint::Zone0Point1)),
            0x34 => Ok(Instruction::Shc(ZonePoint::Zone1Point2)),
            0x35 => Ok(Instruction::Shc(ZonePoint::Zone0Point1)),
            0x36 => Ok(Instruction::Shz(ZonePoint::Zone1Point2)),
            0x37 => Ok(Instruction::Shz(ZonePoint::Zone0Point1)),
            0x38 => Ok(Instruction::Shpix),
            0x3a | 0x3b => Ok(Instruction::Msirp(SetRP0(op == 0x3b))),
            0x2e | 0x2f => Ok(Instruction::Mdap(ShouldRound(op == 0x2f))),
            0x3e | 0x3f => Ok(Instruction::Miap(ShouldRound(op == 0x3f))),
            0xc0...0xdf => {
                Ok(Instruction::Mdrp(SetRP0((op & 0b10000) != 0),
                                     ApplyMinimumDistance((op & 0b01000) != 0),
                                     ShouldRound((op & 0b00100) != 0),
                                     try!(DistanceType::parse(op & 0b00011))))
            }
            0x3c => Ok(Instruction::Alignrp),
            0x0f => Ok(Instruction::Isect),
            0x27 => Ok(Instruction::Alignpts),
            0x39 => Ok(Instruction::Ip),
            0x29 => Ok(Instruction::Utp),
            0x30 => Ok(Instruction::Iup(Axis::Y)),
            0x31 => Ok(Instruction::Iup(Axis::X)),
            0x5d => Ok(Instruction::Deltap1),
            0x71 => Ok(Instruction::Deltap2),
            0x72 => Ok(Instruction::Deltap3),
            0x73 => Ok(Instruction::Deltac1),
            0x74 => Ok(Instruction::Deltac2),
            0x75 => Ok(Instruction::Deltac3),
            0x20 => Ok(Instruction::Dup),
            0x21 => Ok(Instruction::Pop),
            0x22 => Ok(Instruction::Clear),
            0x23 => Ok(Instruction::Swap),
            0x24 => Ok(Instruction::Depth),
            0x25 => Ok(Instruction::Cindex),
            0x26 => Ok(Instruction::Mindex),
            0x8a => Ok(Instruction::Roll),
            0x58 => Ok(Instruction::If),
            0x1b => Ok(Instruction::Else),
            0x59 => Ok(Instruction::EIf),
            0x78 => Ok(Instruction::Jrot),
            0x1c => Ok(Instruction::Jmpr),
            0x79 => Ok(Instruction::Jrof),
            0x50 => Ok(Instruction::Lt),
            0x51 => Ok(Instruction::Lteq),
            0x52 => Ok(Instruction::Gt),
            0x53 => Ok(Instruction::Gteq),
            0x54 => Ok(Instruction::Eq),
            0x55 => Ok(Instruction::Neq),
            0x56 => Ok(Instruction::Odd),
            0x57 => Ok(Instruction::Even),
            0x5a => Ok(Instruction::And),
            0x5b => Ok(Instruction::Or),
            0x5c => Ok(Instruction::Not),
            0x60 => Ok(Instruction::Add),
            0x61 => Ok(Instruction::Sub),
            0x62 => Ok(Instruction::Div),
            0x63 => Ok(Instruction::Mul),
            0x64 => Ok(Instruction::Abs),
            0x65 => Ok(Instruction::Neg),
            0x66 => Ok(Instruction::Floor),
            0x67 => Ok(Instruction::Ceiling),
            0x8b => Ok(Instruction::Max),
            0x8c => Ok(Instruction::Min),
            0x68...0x6b => Ok(Instruction::Round(try!(DistanceType::parse(op & 0b11)))),
            0x6c...0x6f => Ok(Instruction::Nround(try!(DistanceType::parse(op & 0b11)))),
            0x2c => Ok(Instruction::Fdef),
            0x2d => Ok(Instruction::Endf),
            0x2b => Ok(Instruction::Call),
            0x2a => Ok(Instruction::Loopcall),
            0x89 => Ok(Instruction::Idef),
            0x4f => Ok(Instruction::Debug),
            0x88 => Ok(Instruction::Getinfo),
            0x91 => Ok(Instruction::Getvariation),
            _ => Err(ParseError::UnknownOpcode),
        }
    }
}

fn get(data: &[u8], pc: &mut usize) -> Option<u8> {
    match data.get(*pc) {
        Some(&byte) => {
            *pc += 1;
            Some(byte)
        }
        None => None,
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Axis {
    Y = 0,
    X = 1,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum LineOrientation {
    Parallel = 0,
    Perpendicular = 1,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum WhichPosition {
    // Use the current position.
    Current = 0,
    // Use the position in the original outline.
    Original = 1,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum ZonePoint {
    Zone1Point2 = 0,
    Zone0Point1 = 1,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct SetRP0(pub bool);

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ApplyMinimumDistance(pub bool);

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ShouldRound(pub bool);

// See `MDRP` (ttinst2.doc, 277)
#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum DistanceType {
    Gray = 0,
    Black = 1,
    White = 2,
}

impl DistanceType {
    fn parse(value: u8) -> Result<DistanceType, ParseError> {
        match value {
            0 => Ok(DistanceType::Gray),
            1 => Ok(DistanceType::Black),
            2 => Ok(DistanceType::White),
            _ => Err(ParseError::InvalidDistanceType),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ParseError {
    /// The instruction stream terminated normally.
    Eof,
    /// The instruction stream terminated abnormally.
    UnexpectedEof,
    /// An unexpected opcode was encountered.
    UnknownOpcode,
    /// An unexpected value was encountered for `DistanceType`.
    InvalidDistanceType,
}

