// pathfinder/demo2/tiling.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Point2D} from "./geometry";
import {panic, unwrapNull} from "./util";

export interface SVGPath {
    abs(): SVGPath;
    translate(x: number, y: number): SVGPath;
    matrix(m: number[]): SVGPath;
    iterate(f: (segment: string[], index: number, x: number, y: number) => string[][] | void):
            SVGPath;
}

const SVGPath: (path: string) => SVGPath = require('svgpath');

interface EndpointIndex {
    subpathIndex: number;
    endpointIndex: number;
};

export class Tiler {
    private path: SVGPath;
    private endpoints: SubpathEndpoints[];
    private sortedEndpointIndices: EndpointIndex[];

    constructor(path: SVGPath) {
        this.path = path;

        // Accumulate endpoints.
        this.endpoints = [];
        let currentSubpathEndpoints = new SubpathEndpoints;
        path.iterate(segString => {
            const segment = new PathSegment(segString);
            switch (segment.command) {
            case 'M':
                if (!currentSubpathEndpoints.isEmpty()) {
                    this.endpoints.push(currentSubpathEndpoints);
                    currentSubpathEndpoints = new SubpathEndpoints;
                }
                currentSubpathEndpoints.endpoints.push(segment.points[0]);
                break;
            case 'L':
            case 'S':
            case 'Q':
            case 'C':
                // TODO(pcwalton): Canonicalize 'S'.
                currentSubpathEndpoints.endpoints.push(unwrapNull(segment.to()));
                break;
            case 'Z':
                break;
            default:
                panic("Unexpected path command: " + segment.command);
                break;
            }
        });
        if (!currentSubpathEndpoints.isEmpty())
            this.endpoints.push(currentSubpathEndpoints);

        // Sort endpoints.
        this.sortedEndpointIndices = [];
        for (let subpathIndex = 0; subpathIndex < this.endpoints.length; subpathIndex++) {
            const subpathEndpoints = this.endpoints[subpathIndex];
            for (let endpointIndex = 0;
                 endpointIndex < subpathEndpoints.endpoints.length;
                 endpointIndex++) {
                this.sortedEndpointIndices.push({subpathIndex, endpointIndex});
            }
        }
    }

    tile(): void {
        const activeEdges = [];
        for (const endpointIndex of this.sortedEndpointIndices) {

        }
    }
}

class SubpathEndpoints {
    endpoints: Point2D[];

    constructor() {
        this.endpoints = [];
    }

    isEmpty(): boolean {
        return this.endpoints.length < 2;
    }

    prevEndpointIndexOf(index: number): number {
        const prevIndex = index - 1;
        return prevIndex < 0 ? this.endpoints.length - 1 : prevIndex;
    }

    nextEndpointIndexOf(index: number): number {
        const nextIndex = index + 1;
        return nextIndex >= this.endpoints.length ? 0 : nextIndex;
    }

    prevEndpointOf(index: number): Point2D {
        return this.endpoints[this.prevEndpointIndexOf(index)];
    }

    nextEndpointOf(index: number): Point2D {
        return this.endpoints[this.nextEndpointIndexOf(index)];
    }
}

export class PathSegment {
    command: string;
    points: Point2D[];

    constructor(segment: string[]) {
        const points = [];
        for (let i = 1; i < segment.length; i += 2)
            points.push(new Point2D(parseFloat(segment[i]), parseFloat(segment[i + 1])));
        this.points = points;
        this.command = segment[0];
    }

    to(): Point2D | null {
        return this.points[this.points.length - 1];
    }
}
