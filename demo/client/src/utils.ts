// pathfinder/client/src/utils.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

export const UINT32_MAX: number = 0xffffffff;
export const UINT32_SIZE: number = 4;

export function panic(message: string): never {
    throw new PathfinderError(message);
}

export function assert(value: boolean, message: string) {
    if (!value)
        panic(message);
}

export function expectNotNull<T>(value: T | null, message: string): T {
    if (value === null)
        throw new PathfinderError(message);
    return value;
}

function expectNotUndef<T>(value: T | undefined, message: string): T {
    if (value === undefined)
        throw new PathfinderError(message);
    return value;
}

export function unwrapNull<T>(value: T | null): T {
    return expectNotNull(value, "Unexpected null!");
}

export function unwrapUndef<T>(value: T | undefined): T {
    return expectNotUndef(value, "Unexpected `undefined`!");
}

export function scaleRect(rect: glmatrix.vec4, scale: number): glmatrix.vec4 {
    const upperLeft = glmatrix.vec2.clone([rect[0], rect[1]]);
    const lowerRight = glmatrix.vec2.clone([rect[2], rect[3]]);
    glmatrix.vec2.scale(upperLeft, upperLeft, scale);
    glmatrix.vec2.scale(lowerRight, lowerRight, scale);
    return glmatrix.vec4.clone([upperLeft[0], upperLeft[1], lowerRight[0], lowerRight[1]]);
}

export class PathfinderError extends Error {
    constructor(message?: string | undefined) {
        super(message);
    }
}
