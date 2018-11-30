// pathfinder/demo2/geometry.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

export const EPSILON: number = 1e-6;

export class Point2D {
    x: number;
    y: number;

    constructor(x: number, y: number) {
        this.x = x;
        this.y = y;
        Object.freeze(this);
    }

    approxEq(other: Point2D): boolean {
        return approxEq(this.x, other.x) && approxEq(this.y, other.y);
    }

    lerp(other: Point2D, t: number): Point2D {
        return new Point2D(lerp(this.x, other.x, t), lerp(this.y, other.y, t));
    }
}

export interface Size2D {
    width: number;
    height: number;
}

export class Rect {
    origin: Point2D;
    size: Size2D;

    constructor(origin: Point2D, size: Size2D) {
        this.origin = origin;
        this.size = size;
        Object.freeze(this);
    }

    unionWithPoint(point: Point2D): Rect {
        let newOrigin = this.origin, newSize = this.size;

        if (point.x < this.origin.x) {
            newSize = {
                width: newSize.width + newOrigin.x - point.x,
                height: newSize.height,
            };
            newOrigin = new Point2D(point.x, newOrigin.y);
        } else if (point.x > this.maxX()) {
            newSize = {
                width: newSize.width + point.x - this.maxX(),
                height: newSize.height,
            };
        }

        if (point.y < this.origin.y) {
            newSize = {
                width: newSize.width,
                height: newSize.height + newOrigin.y - point.y,
            };
            newOrigin = new Point2D(newOrigin.x, point.y);
        } else if (point.y > this.maxY()) {
            newSize = {
                width: newSize.width,
                height: newSize.height + point.y - this.maxY(),
            };
        }

        return new Rect(newOrigin, newSize);
    }

    maxX(): number {
        return this.origin.x + this.size.width;
    }

    maxY(): number {
        return this.origin.y + this.size.height;
    }
}

export interface Vector3D {
    x: number;
    y: number;
    z: number;
}

export class Matrix2D {
    a: number; b: number;
    c: number; d: number;
    tx: number; ty: number;

    constructor(a: number, b: number, c: number, d: number, tx: number, ty: number) {
        this.a = a; this.b = b;
        this.c = c; this.d = d;
        this.tx = tx; this.ty = ty;
    }
}

export function approxEq(a: number, b: number): boolean {
    return Math.abs(a - b) <= EPSILON;
}

export function lerp(a: number, b: number, t: number): number {
    return a + (b - a) * t;
}

export function cross(a: Vector3D, b: Vector3D): Vector3D {
    return {
        x: a.y*b.z - a.z*b.y,
        y: a.z*b.x - a.x*b.z,
        z: a.x*b.y - a.y*b.x,
    };
}
