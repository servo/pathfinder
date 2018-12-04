// pathfinder/demo2/path-utils.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Point2D} from "./geometry";
import {SVGPath} from "./tiling";

const SVGPath: (path: string) => SVGPath = require('svgpath');

export function flattenPath(path: SVGPath): SVGPath {
    return path.abs().iterate(segment => {
        if (segment[0] === 'C') {
            const ctrl0 = new Point2D(parseFloat(segment[segment.length - 6]),
                                      parseFloat(segment[segment.length - 5]));
            const ctrl1 = new Point2D(parseFloat(segment[segment.length - 4]),
                                      parseFloat(segment[segment.length - 3]));
            const to = new Point2D(parseFloat(segment[segment.length - 2]),
                                   parseFloat(segment[segment.length - 1]));
            const ctrl = new Point2D(0.5 * (ctrl0.x + ctrl1.x), 0.5 * (ctrl0.y + ctrl1.y));
            return [['Q', "" + ctrl.x, "" + ctrl.y, "" + to.x, "" + to.y]];
        }
        if (segment[0] === 'A') {
            const to = new Point2D(parseFloat(segment[segment.length - 2]),
                                   parseFloat(segment[segment.length - 1]));
            return [['L', "" + to.x, "" + to.y]];
        }
        return [segment];
    });
}

export function canonicalizePath(path: SVGPath): SVGPath {
    return path.abs().iterate(segment => {
        if (segment[0] === 'H')
            return [['L', segment[1], '0']];
        if (segment[0] === 'V')
            return [['L', '0', segment[1]]];
        return [segment];
    });
}

