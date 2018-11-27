// pathfinder/demo2/util.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

export function panic(msg: string): never {
    throw new Error(msg);
}

export function staticCast<T>(value: any, constructor: { new(...args: any[]): T }): T {
    if (!(value instanceof constructor))
        panic("Invalid dynamic cast");
    return value;
}

export function unwrapNull<T>(value: T | null): T {
    if (value == null)
        throw new Error("Unexpected null");
    return value;
}
