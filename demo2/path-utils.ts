// pathfinder/demo2/path-utils.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Point2D, Rect, EPSILON} from "./geometry";
import {SVGPath, Edge} from "./tiling";
import { ENGINE_METHOD_DIGESTS } from "constants";
import { AssertionError } from "assert";
import { unwrapNull, unwrapUndef } from "./util";

const SVGPath: (path: string) => SVGPath = require('svgpath');

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

    toStringPieces(): string[] {
        const pieces = [this.command];
        for (const point of this.points) {
            pieces.push(" " + point.x);
            pieces.push(" " + point.y);
        }
        return pieces;
    }

    toString(): string {
        return this.toStringPieces().join(" ");
    }
}

export function flattenPath(path: SVGPath): SVGPath {
    let lastPoint: Point2D | null = null;
    return path.unshort().abs().iterate(segmentPieces => {
        let segment = new PathSegment(segmentPieces);
        if (segment.command === 'C' && lastPoint != null) {
            const cubicEdge = new CubicEdge(lastPoint,
                                            segment.points[0],
                                            segment.points[1],
                                            segment.points[2]);
            //console.log("cubic edge", cubicEdge);
            const edges: Edge[] = cubicEdge.toQuadraticEdges();
            /*const edges: Edge[] = [
                new Edge(lastPoint,
                         segment.points[0].lerp(segment.points[1], 0.5),
                         segment.points[2]),
            ];*/
            const newSegments = edges.map(edge => edge.toSVGPieces());
            //console.log("... resulting new segments:", newSegments);
            lastPoint = segment.to();
            return newSegments;
        }
        if (segment.command === 'H' && lastPoint != null)
            segment = new PathSegment(['L', segmentPieces[1], "" + lastPoint.y]);
        if (segment.command === 'V' && lastPoint != null)
            segment = new PathSegment(['L', "" + lastPoint.x, segmentPieces[1]]);
        lastPoint = segment.to();
        return [segment.toStringPieces()];
    });
}

export function makePathMonotonic(path: SVGPath): SVGPath {
    let lastPoint: Point2D | null = null;
    return path.iterate(segmentPieces => {
        let segment = new PathSegment(segmentPieces);
        if (segment.command === 'Q' && lastPoint != null) {
            const edge = new Edge(lastPoint, segment.points[0], segment.points[1]);
            const minX = Math.min(edge.from.x, edge.to.x);
            const maxX = Math.max(edge.from.x, edge.to.x);

            const edgesX: Edge[] = [];
            if (edge.ctrl!.x < minX || edge.ctrl!.x > maxX) {
                const t = (edge.from.x - edge.ctrl!.x) /
                    (edge.from.x - 2.0 * edge.ctrl!.x + edge.to.x);
                const subdivided = edge.subdivideAt(t);
                if (t < -EPSILON || t > 1.0 + EPSILON)
                    throw new Error("Bad t value when making monotonic X!");
                edgesX.push(subdivided.prev, subdivided.next);
            } else {
                edgesX.push(edge);
            }

            const newEdges = [];
            for (const edge of edgesX) {
                const minY = Math.min(edge.from.y, edge.to.y);
                const maxY = Math.max(edge.from.y, edge.to.y);

                if (edge.ctrl!.y < minY || edge.ctrl!.y > maxY) {
                    const t = (edge.from.y - edge.ctrl!.y) /
                        (edge.from.y - 2.0 * edge.ctrl!.y + edge.to.y);
                    if (t < -EPSILON || t > 1.0 + EPSILON)
                        throw new Error("Bad t value when making monotonic Y!");
                    const subdivided = edge.subdivideAt(t);
                    newEdges.push(subdivided.prev, subdivided.next);
                } else {
                    newEdges.push(edge);
                }
            }

            lastPoint = segment.to();
            return newEdges.map(newEdge => newEdge.toSVGPieces());
        }

        lastPoint = segment.to();
        return [segment.toStringPieces()];
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

class CubicEdge {
    from: Point2D;
    ctrl0: Point2D;
    ctrl1: Point2D;
    to: Point2D;

    constructor(from: Point2D, ctrl0: Point2D, ctrl1: Point2D, to: Point2D) {
        this.from = from;
        this.ctrl0 = ctrl0;
        this.ctrl1 = ctrl1;
        this.to = to;
    }

    subdivideAt(t: number): SubdividedCubicEdges {
        const p0 = this.from, p1 = this.ctrl0, p2 = this.ctrl1, p3 = this.to;
        const p01 = p0.lerp(p1, t), p12 = p1.lerp(p2, t), p23 = p2.lerp(p3, t);
        const p012 = p01.lerp(p12, t), p123 = p12.lerp(p23, t);
        const p0123 = p012.lerp(p123, t);
        return {
            prev: new CubicEdge(p0, p01, p012, p0123),
            next: new CubicEdge(p0123, p123, p23, p3),
        };
    }

    toQuadraticEdges(): Edge[] {
        const MAX_APPROXIMATION_ITERATIONS: number = 32;
        const TOLERANCE: number = 0.1;

        const results = [], worklist: CubicEdge[] = [this];
        while (worklist.length > 0) {
            let current = unwrapUndef(worklist.pop());
            for (let iteration = 0; iteration < MAX_APPROXIMATION_ITERATIONS; iteration++) {
                const deltaCtrl0 = current.from.sub(current.ctrl0.scale(3.0))
                                               .add(current.ctrl1.scale(3.0).sub(current.to));
                const deltaCtrl1 = current.ctrl0.scale(3.0)
                                                .sub(current.from)
                                                .add(current.to.sub(current.ctrl1.scale(3.0)));
                const maxError = Math.max(deltaCtrl0.length(), deltaCtrl1.length()) / 6.0;
                if (maxError < TOLERANCE)
                    break;

                const subdivided = current.subdivideAt(0.5);
                worklist.push(subdivided.next);
                current = subdivided.prev;
            }

            const approxCtrl0 = current.ctrl0.scale(3.0).sub(current.from).scale(0.5);
            const approxCtrl1 = current.ctrl1.scale(3.0).sub(current.to).scale(0.5);
            results.push(new Edge(current.from, approxCtrl0.lerp(approxCtrl1, 0.5), current.to));
        }

        return results;
    }
}

interface SubdividedCubicEdges {
    prev: CubicEdge;
    next: CubicEdge;
}
