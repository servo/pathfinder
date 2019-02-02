// pathfinder/simd/src/test.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::default::{F32x4, I32x4, U32x4};

// F32x4

#[test]
fn test_f32x4_constructors() {
    let a = F32x4::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!((a[0], a[1], a[2], a[3]), (1.0, 2.0, 3.0, 4.0));
    let b = F32x4::splat(10.0);
    assert_eq!(b, F32x4::new(10.0, 10.0, 10.0, 10.0));
}

#[test]
fn test_f32x4_accessors_and_mutators() {
    let a = F32x4::new(5.0, 6.0, 7.0, 8.0);
    assert_eq!((a.x(), a.y(), a.z(), a.w()), (5.0, 6.0, 7.0, 8.0));
    let mut b = F32x4::new(10.0, 11.0, 12.0, 13.0);
    b.set_x(20.0);
    b.set_y(30.0);
    b.set_z(40.0);
    b.set_w(50.0);
    assert_eq!(b, F32x4::new(20.0, 30.0, 40.0, 50.0));
}

#[test]
fn test_f32x4_basic_ops() {
    let a = F32x4::new(1.0, 3.0, 5.0, 7.0);
    let b = F32x4::new(2.0, 2.0, 6.0, 6.0);
    assert_eq!(a.min(b), F32x4::new(1.0, 2.0, 5.0, 6.0));
    assert_eq!(a.max(b), F32x4::new(2.0, 3.0, 6.0, 7.0));
    let c = F32x4::new(-1.0, 1.0, -20.0, 3.0);
    assert_eq!(c.abs(), F32x4::new(1.0, 1.0, 20.0, 3.0));
}

#[test]
fn test_f32x4_packed_comparisons() {
    let a = F32x4::new(7.0,  3.0, 6.0, -2.0);
    let b = F32x4::new(10.0, 3.0, 5.0, -2.0);
    assert_eq!(a.packed_eq(b), U32x4::new(0, !0, 0, !0));
}

// TODO(pcwalton): This should test them all!
#[test]
fn test_f32x4_swizzles() {
    let a = F32x4::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(a.xxxx(), F32x4::splat(1.0));
    assert_eq!(a.yyyy(), F32x4::splat(2.0));
    assert_eq!(a.zzzz(), F32x4::splat(3.0));
    assert_eq!(a.wwww(), F32x4::splat(4.0));
    assert_eq!(a.xyxy(), F32x4::new(1.0, 2.0, 1.0, 2.0));
    assert_eq!(a.yzzy(), F32x4::new(2.0, 3.0, 3.0, 2.0));
    assert_eq!(a.wzyx(), F32x4::new(4.0, 3.0, 2.0, 1.0));
    assert_eq!(a.ywzx(), F32x4::new(2.0, 4.0, 3.0, 1.0));
    assert_eq!(a.wzwz(), F32x4::new(4.0, 3.0, 4.0, 3.0));
}

#[test]
fn test_f32x4_concatenations() {
    let a = F32x4::new(4.0,   2.0,  6.0, -1.0);
    let b = F32x4::new(10.0, -3.0, 15.0, 41.0);
    assert_eq!(a.concat_xy_xy(b), F32x4::new( 4.0,  2.0, 10.0, -3.0));
    assert_eq!(a.concat_xy_zw(b), F32x4::new( 4.0,  2.0, 15.0, 41.0));
    assert_eq!(a.concat_zw_zw(b), F32x4::new( 6.0, -1.0, 15.0, 41.0));
    assert_eq!(a.concat_wz_yx(b), F32x4::new(-1.0,  6.0, -3.0, 10.0));
}

#[test]
fn test_f32x4_arithmetic_overloads() {
    let a         = F32x4::new(4.0, -1.0,  6.0, -32.0);
    let b         = F32x4::new(0.5,  0.5, 10.0,   3.0);
    let a_plus_b  = F32x4::new(4.5, -0.5, 16.0, -29.0);
    let a_minus_b = F32x4::new(3.5, -1.5, -4.0, -35.0);
    let a_times_b = F32x4::new(2.0, -0.5, 60.0, -96.0);
    assert_eq!(a + b, a_plus_b);
    assert_eq!(a - b, a_minus_b);
    assert_eq!(a * b, a_times_b);
    let mut c = a;
    c += b;
    assert_eq!(c, a_plus_b);
    c = a;
    c -= b;
    assert_eq!(c, a_minus_b);
    c = a;
    c *= b;
    assert_eq!(c, a_times_b);
    assert_eq!(-a, F32x4::new(-4.0, 1.0, -6.0, 32.0));
}

#[test]
fn test_f32x4_index_overloads() {
    let mut a = F32x4::new(4.0, 1.0, -32.5, 75.0);
    assert_eq!(a[2], -32.5);
    a[3] = 300.0;
    assert_eq!(a[3], 300.0);
    a[0] *= 0.5;
    assert_eq!(a[0], 2.0);
}

#[test]
fn test_f32x4_conversions() {
    let a = F32x4::new(48.0, -4.0, 200.0, 7.0);
    assert_eq!(a.to_i32x4(), I32x4::new(48, -4, 200, 7));
}

// I32x4

#[test]
fn test_i32x4_constructors() {
    let a = I32x4::new(3, 58, 10, 4);
    assert_eq!((a[0], a[1], a[2], a[3]), (3, 58, 10, 4));
    let b = I32x4::splat(39);
    assert_eq!(b, I32x4::new(39, 39, 39, 39));
}

#[test]
fn test_i32x4_basic_ops() {
    let a = I32x4::new(6,  29, -40, 2 );
    let b = I32x4::new(10, -5,  10, 46);
    assert_eq!(a.min(b), I32x4::new(6, -5, -40, 2));
}

#[test]
fn test_i32x4_packed_comparisons() {
    let a = I32x4::new( 59, 1, 5, 63 );
    let b = I32x4::new(-59, 1, 5, 104);
    assert_eq!(a.packed_eq(b), U32x4::new(0, !0, !0, 0));
}
