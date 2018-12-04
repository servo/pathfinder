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

export class PathSegment {
    command: string;
    points: Point2D[];

    constructor(segment: string[]) {
        const points = [];
        for (let i = 1; i < segment.length; i += 2)
            points.push(new Point2D(parseFloat(segment[i]), parseFloat(segment[i + 1])));
        this.points = points;
        //console.log("PathSegment, segment=", segment, "points=", points);
        this.command = segment[0];
    }

    to(): Point2D | null {
        return this.points[this.points.length - 1];
    }
}

export function flattenPath(path: SVGPath): SVGPath {
    return path.unshort().abs().iterate(segment => {
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
    return path.unshort().abs().iterate(segment => {
        if (segment[0] === 'H')
            return [['L', segment[1], '0']];
        if (segment[0] === 'V')
            return [['L', '0', segment[1]]];
        return [segment];
    });
}

export class Outline {
    suboutlines: Suboutline[];

    constructor(path: SVGPath) {
        this.suboutlines = [];
        let suboutline = new Suboutline;
        path.iterate(segmentPieces => {
            const segment = new PathSegment(segmentPieces);
            if (segment.command === 'M') {
                if (!suboutline.isEmpty()) {
                    this.suboutlines.push(suboutline);
                    suboutline = new Suboutline;
                }
            }
            for (let pointIndex = 0; pointIndex < segment.points.length; pointIndex++) {
                suboutline.points.push(new OutlinePoint(segment.points[pointIndex],
                                                        pointIndex < segment.points.length - 1));
            }
        });
        if (!suboutline.isEmpty())
            this.suboutlines.push(suboutline);
    }

    calculateNormals(): void {
        for (const suboutline of this.suboutlines)
            suboutline.calculateNormals();
    }

    stroke(radius: number): void {
        for (const suboutline of this.suboutlines)
            suboutline.stroke(radius);
    }

    toSVGPathString(): string {
        return this.suboutlines.map(suboutline => suboutline.toSVGPathString()).join(" ");
    }
}

export class Suboutline {
    points: OutlinePoint[];
    normals: Point2D[] | null;

    constructor() {
        this.points = [];
        this.normals = null;
    }

    isEmpty(): boolean {
        return this.points.length === 0;
    }

    calculateNormals(): void {
        this.normals = [];
        for (let pointIndex = 0; pointIndex < this.points.length; pointIndex++) {
            const prevPointIndex = pointIndex === 0 ? this.points.length - 1 : pointIndex - 1;
            const nextPointIndex = pointIndex === this.points.length - 1 ? 0 : pointIndex + 1;
            const prevPoint = this.points[prevPointIndex].position;
            const point = this.points[pointIndex].position;
            const nextPoint = this.points[nextPointIndex].position;
            let prevVector = prevPoint.sub(point), nextVector = nextPoint.sub(point);
            this.normals.push(prevVector.add(nextVector).normalize());
        }
    }

    stroke(radius: number): void {
        if (this.normals == null)
            throw new Error("Calculate normals first!");
        const newPoints = [];
        for (let pointIndex = 0; pointIndex < this.points.length; pointIndex++) {
            const point = this.points[pointIndex], normal = this.normals[pointIndex];
            const newPosition = point.position.sub(normal.scale(radius));
            newPoints.push(new OutlinePoint(newPosition, point.offCurve));
        }
        for (let pointIndex = this.points.length - 1; pointIndex >= 0; pointIndex--) {
            const point = this.points[pointIndex], normal = this.normals[pointIndex];
            const newPosition = point.position.add(normal.scale(radius));
            newPoints.push(new OutlinePoint(newPosition, point.offCurve));
        }
        this.points = newPoints;
        this.normals = null;
    }

    toSVGPathString(): string {
        let string = "";
        const queuedPositions = [];
        for (let pointIndex = 0; pointIndex < this.points.length; pointIndex++) {
            const point = this.points[pointIndex];
            queuedPositions.push(point.position);
            if (pointIndex > 0 && point.offCurve)
                continue;
            let command: string;
            if (pointIndex === 0)
                command = 'M';
            else if (queuedPositions.length === 1)
                command = 'L';
            else
                command = 'Q';
            string += command + " ";
            for (const position of queuedPositions)
                string += position.x + " " + position.y + " ";
            queuedPositions.splice(0);
        }
        string += "Z";
        return string;
    }
}

export class OutlinePoint {
    position: Point2D;
    offCurve: boolean;

    constructor(position: Point2D, offCurve: boolean) {
        this.position = position;
        this.offCurve = offCurve;
    }
}
