// pathfinder/demo2/tiling.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Point2D, Rect, Size2D, cross} from "./geometry";
import {panic, staticCast, unwrapNull} from "./util";

export const TILE_SIZE: Size2D = {width: 32.0, height: 32.0};

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
    private sortedEdges: Edge[];
    private boundingRect: Rect | null;
    private strips: Strip[];
    private tileStrips: TileStrip[];

    constructor(path: SVGPath) {
        this.path = path;
        //console.log("tiler: path=", path);

        // Accumulate endpoints.
        this.endpoints = [];
        let currentSubpathEndpoints = new SubpathEndpoints;
        path.iterate(segString => {
            const segment = new PathSegment(segString);
            //console.log("segment", segment);
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

        // Sort edges, and accumulate bounding rect.
        this.sortedEdges = [];
        this.boundingRect = null;
        for (let subpathIndex = 0; subpathIndex < this.endpoints.length; subpathIndex++) {
            const subpathEndpoints = this.endpoints[subpathIndex];
            for (let endpointIndex = 0;
                 endpointIndex < subpathEndpoints.endpoints.length;
                 endpointIndex++) {
                this.sortedEdges.push(this.nextEdgeFromEndpoint({subpathIndex, endpointIndex}));

                const endpoint = subpathEndpoints.endpoints[endpointIndex];
                if (this.boundingRect == null)
                    this.boundingRect = new Rect(endpoint, {width: 0, height: 0});
                else
                    this.boundingRect = this.boundingRect.unionWithPoint(endpoint);
            }
        }
        this.sortedEdges.sort((edgeA, edgeB) => {
            return Math.min(edgeA.from.y, edgeA.to.y) - Math.min(edgeB.from.y, edgeB.to.y);
        });

        /*
        // Dump endpoints.
        const allEndpoints = this.sortedEndpointIndices.map(index => {
            return {
                index: index,
                endpoint: this.endpoints[index.subpathIndex].endpoints[index.endpointIndex],
            };
        });
        //console.log("allEndpoints", allEndpoints);
        */

        this.strips = [];
        this.tileStrips = [];
    }

    tile(): void {
        if (this.boundingRect == null)
            return;

        const activeIntervals = new Intervals(this.boundingRect.maxX());;
        let activeEdges: Edge[] = [];
        let nextEdgeIndex = 0;
        this.strips = [];

        let tileTop = this.boundingRect.origin.y - this.boundingRect.origin.y % TILE_SIZE.height;
        while (tileTop < this.boundingRect.maxY()) {
            const tileBottom = tileTop + TILE_SIZE.height;

            // Populate tile strip with active intervals.
            const strip = new Strip(tileTop);
            /*console.log("tileTop", tileTop,
                        "intervals", JSON.stringify(activeIntervals.intervalRanges()));*/
            for (const interval of activeIntervals.intervalRanges()) {
                if (interval.winding === 0)
                    continue;
                const startPoint = new Point2D(interval.start, tileTop);
                const endPoint = new Point2D(interval.end, tileTop);
                if (interval.winding < 0)
                    strip.pushEdge(new Edge(startPoint, endPoint));
                else
                    strip.pushEdge(new Edge(endPoint, startPoint));
            }

            // Populate tile strip with active edges.
            const oldEdges = activeEdges;
            activeEdges = [];
            for (const activeEdge of oldEdges)
                this.processEdgeY(activeEdge, strip, activeEdges, activeIntervals, tileTop);

            while (nextEdgeIndex < this.sortedEdges.length) {
                const edge = this.sortedEdges[nextEdgeIndex];
                if (edge.from.y > tileBottom && edge.to.y > tileBottom)
                    break;

                this.processEdgeY(edge, strip, activeEdges, activeIntervals, tileTop);
                //console.log("new intervals:", JSON.stringify(activeIntervals));
                nextEdgeIndex++;
            }

            this.strips.push(strip);
            tileTop = tileBottom;
        }

        // Cut up tile strips.
        //console.log("strips count:", this.strips.length);
        this.tileStrips = this.strips.map(strip => this.divideStrip(strip));
    }

    private divideStrip(strip: Strip): TileStrip {
        // Sort edges.
        const sortedEdges = strip.edges.slice(0);
        sortedEdges.sort((edgeA, edgeB) => {
            return Math.min(edgeA.from.x, edgeA.to.x) - Math.min(edgeB.from.x, edgeB.to.x);
        });

        const tileStrip = new TileStrip(strip.tileTop);
        const boundingRect = unwrapNull(this.boundingRect);
        let tileLeft = boundingRect.origin.x - boundingRect.origin.x % TILE_SIZE.width;
        let activeEdges: Edge[] = [];
        let nextEdgeIndex = 0;

        while (tileLeft < boundingRect.maxX()) {
            const tile = new Tile(tileLeft);
            const tileRight = tileLeft + TILE_SIZE.width;

            // Populate tile with active edges.
            const oldEdges = activeEdges;
            activeEdges = [];
            for (const activeEdge of oldEdges)
                this.processEdgeX(activeEdge, tile, activeEdges);

            while (nextEdgeIndex < sortedEdges.length) {
                const edge = sortedEdges[nextEdgeIndex];
                if (edge.from.x > tileRight && edge.to.x > tileRight)
                    break;

                this.processEdgeX(edge, tile, activeEdges);
                nextEdgeIndex++;
            }

            tileStrip.pushTile(tile);
            tileLeft = tileRight;
        }

        return tileStrip;
    }

    getStrips(): Strip[] {
        return this.strips;
    }

    getTileStrips(): TileStrip[] {
        return this.tileStrips;
    }

    getBoundingRect(): Rect {
        if (this.boundingRect == null)
            return new Rect(new Point2D(0, 0), {width: 0, height: 0});

        const tileLeft = this.boundingRect.origin.x - this.boundingRect.origin.x % TILE_SIZE.width;
        const tileRight = Math.ceil(this.boundingRect.maxX() / TILE_SIZE.width) * TILE_SIZE.width;
        const tileTop = this.boundingRect.origin.y - this.boundingRect.origin.y % TILE_SIZE.height;
        const tileBottom = Math.ceil(this.boundingRect.maxY() / TILE_SIZE.height) *
            TILE_SIZE.height;
        return new Rect(new Point2D(tileLeft, tileTop),
                        {width: tileRight - tileLeft, height: tileBottom - tileTop});
    }

    private processEdgeX(edge: Edge, tile: Tile, activeEdges: Edge[]): void {
        const tileRight = tile.tileLeft + TILE_SIZE.width;
        const clipped = this.clipEdgeX(edge, tileRight);

        if (clipped.left != null)
            tile.pushEdge(clipped.left);

        if (clipped.right != null)
            activeEdges.push(clipped.right);
    }

    private processEdgeY(edge: Edge,
                         tileStrip: Strip,
                         activeEdges: Edge[],
                         intervals: Intervals,
                         tileTop: number):
                         void {
        const tileBottom = tileTop + TILE_SIZE.height;
        const clipped = this.clipEdgeY(edge, tileBottom);

        if (clipped.upper != null) {
            //console.log("pushing clipped upper edge:", JSON.stringify(clipped.upper));
            tileStrip.pushEdge(clipped.upper);

            if (clipped.upper.from.x <= clipped.upper.to.x)
                intervals.add(new IntervalRange(clipped.upper.from.x, clipped.upper.to.x, -1));
            else
                intervals.add(new IntervalRange(clipped.upper.to.x, clipped.upper.from.x, 1));
        }

        if (clipped.lower != null)
            activeEdges.push(clipped.lower);
    }

    private clipEdgeX(edge: Edge, x: number): ClippedEdgesX {
        if (edge.from.x < x && edge.to.x < x)
            return {left: edge, right: null};
        if (edge.from.x > x && edge.to.x > x)
            return {left: null, right: edge};

        const from     = {x: edge.from.x, y: edge.from.y, z: 1.0};
        const to       = {x: edge.to.x,   y: edge.to.y,   z: 1.0};
        const clipLine = {x: 1.0,         y: 0.0,         z: -x };

        const intersectionHC = cross(cross(from, to), clipLine);
        const intersection = new Point2D(intersectionHC.x / intersectionHC.z,
                                         intersectionHC.y / intersectionHC.z);
        const fromEdge = new Edge(edge.from, intersection);
        const toEdge = new Edge(intersection, edge.to);

        if (edge.from.x < x)
            return {left: fromEdge, right: toEdge};
        return {left: toEdge, right: fromEdge};
    }

    private clipEdgeY(edge: Edge, y: number): ClippedEdgesY {
        if (edge.from.y < y && edge.to.y < y)
            return {upper: edge, lower: null};
        if (edge.from.y > y && edge.to.y > y)
            return {upper: null, lower: edge};

        const from     = {x: edge.from.x, y: edge.from.y, z: 1.0};
        const to       = {x: edge.to.x,   y: edge.to.y,   z: 1.0};
        const clipLine = {x: 0.0,         y: 1.0,         z: -y };

        const intersectionHC = cross(cross(from, to), clipLine);
        const intersection = new Point2D(intersectionHC.x / intersectionHC.z,
                                         intersectionHC.y / intersectionHC.z);
        const fromEdge = new Edge(edge.from, intersection);
        const toEdge = new Edge(intersection, edge.to);

        if (edge.from.y < y)
            return {upper: fromEdge, lower: toEdge};
        return {upper: toEdge, lower: fromEdge};
    }

    private prevEdgeFromEndpoint(endpointIndex: EndpointIndex): Edge {
        const subpathEndpoints = this.endpoints[endpointIndex.subpathIndex];
        return new Edge(subpathEndpoints.prevEndpointOf(endpointIndex.endpointIndex),
                        subpathEndpoints.endpoints[endpointIndex.endpointIndex]);
    }

    private nextEdgeFromEndpoint(endpointIndex: EndpointIndex): Edge {
        const subpathEndpoints = this.endpoints[endpointIndex.subpathIndex];
        return new Edge(subpathEndpoints.endpoints[endpointIndex.endpointIndex],
                        subpathEndpoints.nextEndpointOf(endpointIndex.endpointIndex));
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
        //console.log("PathSegment, segment=", segment, "points=", points);
        this.command = segment[0];
    }

    to(): Point2D | null {
        return this.points[this.points.length - 1];
    }
}

class Edge {
    from: Point2D;
    to: Point2D;

    constructor(from: Point2D, to: Point2D) {
        this.from = from;
        this.to = to;
        Object.freeze(this);
    }
}

class Strip {
    edges: Edge[];
    tileTop: number;

    constructor(tileTop: number) {
        this.edges = [];
        this.tileTop = tileTop;
    }

    pushEdge(edge: Edge): void {
        this.edges.push(new Edge(new Point2D(edge.from.x, edge.from.y - this.tileTop),
                                 new Point2D(edge.to.x, edge.to.y - this.tileTop)));
    }

    tileBottom(): number {
        return this.tileTop + TILE_SIZE.height;
    }
}

class TileStrip {
    tiles: Tile[];
    tileTop: number;

    constructor(tileTop: number) {
        this.tiles = [];
        this.tileTop = tileTop;
    }

    pushTile(tile: Tile): void {
        this.tiles.push(tile);
    }

    tileBottom(): number {
        return this.tileTop + TILE_SIZE.height;
    }
}

class Tile {
    edges: Edge[];
    tileLeft: number;

    constructor(tileLeft: number) {
        this.edges = [];
        this.tileLeft = tileLeft;
    }

    pushEdge(edge: Edge): void {
        this.edges.push(new Edge(new Point2D(edge.from.x - this.tileLeft, edge.from.y),
                                 new Point2D(edge.to.x - this.tileLeft, edge.to.y)));
    }
}

interface ClippedEdgesX {
    left:  Edge | null;
    right: Edge | null;
}

interface ClippedEdgesY {
    upper: Edge | null;
    lower: Edge | null;
}

class Intervals {
    private ranges: IntervalRange[];

    constructor(width: number) {
        this.ranges = [new IntervalRange(0, width, 0)];
    }

    intervalRanges(): IntervalRange[] {
        return this.ranges;
    }

    add(range: IntervalRange): void {
        //console.log("IntervalRange.add(", range, ")");
        //console.log("... before ...", JSON.stringify(this));

        this.splitAt(range.start);
        this.splitAt(range.end);

        let startIndex = this.ranges.length, endIndex = this.ranges.length;
        for (let i = 0; i < this.ranges.length; i++) {
            if (range.start === this.ranges[i].start)
                startIndex = i;
            if (range.end === this.ranges[i].end)
                endIndex = i + 1;
        }

        // Adjust winding numbers.
        for (let i = startIndex; i < endIndex; i++)
            this.ranges[i].winding += range.winding;

        this.mergeAdjacent();

        //console.log("... after ...", JSON.stringify(this));
    }

    clear(): void {
        this.ranges = [new IntervalRange(0, this.ranges[this.ranges.length - 1].end, 0)];
    }

    private splitAt(value: number): void {
        for (let i = 0; i < this.ranges.length; i++) {
            if (this.ranges[i].start < value && value < this.ranges[i].end) {
                const oldRange = this.ranges[i];
                const range0 = new IntervalRange(oldRange.start, value, oldRange.winding);
                const range1 = new IntervalRange(value, oldRange.end, oldRange.winding);
                this.ranges.splice(i, 1, range0, range1);
                break;
            }
        }
    }

    private mergeAdjacent(): void {
        let i = 0;
        while (i + 1 < this.ranges.length) {
            if (this.ranges[i].end === this.ranges[i + 1].start &&
                    this.ranges[i].winding === this.ranges[i + 1].winding) {
                this.ranges[i].end = this.ranges[i + 1].end;
                this.ranges.splice(i + 1, 1);
                continue;
            }
            i++;
        }
    }
}

class IntervalRange {
    start: number;
    end: number;
    winding: number;

    constructor(start: number, end: number, winding: number) {
        this.start = start;
        this.end = end;
        this.winding = winding;
    }

    contains(value: number): boolean {
        return value >= this.start && value < this.end;
    }
}

// Debugging

const SVG_NS: string = "http://www.w3.org/2000/svg";

export class TileDebugger {
    svg: SVGElement;
    size: Size2D;

    constructor(document: HTMLDocument) {
        this.svg = staticCast(document.createElementNS(SVG_NS, 'svg'), SVGElement);

        this.size = {width: 0, height: 0};

        this.svg.style.position = 'absolute';
        this.svg.style.left = "0";
        this.svg.style.top = "0";
        this.updateSVGSize();
    }

    addTiler(tiler: Tiler, fillColor: string, id: string): void {
        const boundingRect = tiler.getBoundingRect();
        this.size.width = Math.max(this.size.width, boundingRect.maxX());
        this.size.height = Math.max(this.size.height, boundingRect.maxY());

        const tileStrips = tiler.getTileStrips();
        for (let tileStripIndex = 0; tileStripIndex < tileStrips.length; tileStripIndex++) {
            const tileStrip = tileStrips[tileStripIndex];
            const tileBottom = tileStrip.tileBottom();

            for (let tileIndex = 0; tileIndex < tileStrip.tiles.length; tileIndex++) {
                const tile = tileStrip.tiles[tileIndex];

                let path = "";
                for (const edge of tile.edges) {
                    path += "M " + edge.from.x + " " + edge.from.y + " ";
                    path += "L " + edge.to.x + " " + edge.to.y + " ";
                    path += "L " + edge.to.x + " " + TILE_SIZE.height + " ";
                    path += "L " + edge.from.x + " " + TILE_SIZE.height + " ";
                    path += "Z ";
                }

                const pathElement = staticCast(document.createElementNS(SVG_NS, 'path'),
                                               SVGPathElement);
                pathElement.setAttribute('d', path);
                pathElement.setAttribute('fill', fillColor);
                //pathElement.setAttribute('stroke', "rgb(0, 128.0, 0)");
                pathElement.setAttribute('data-tile-id', id);
                pathElement.setAttribute('data-tile-index', "" + tileIndex);
                pathElement.setAttribute('data-tile-strip-index', "" + tileStripIndex);
                pathElement.setAttribute('transform',
                    "translate(" + tile.tileLeft + " " + tileStrip.tileTop + ")");
                this.svg.appendChild(pathElement);
            }
        }

        this.updateSVGSize();
    }

    private updateSVGSize(): void {
        this.svg.style.width = this.size.width + "px";
        this.svg.style.height = this.size.height + "px";
    }
}

function assertEq<T>(actual: T, expected: T): void {
    if (JSON.stringify(expected) !== JSON.stringify(actual)) {
        console.error("expected", expected, "but found", actual);
        throw new Error("Assertion failed!");
    }
}

export function testIntervals(): void {
    const intervals = new Intervals(7);
    intervals.add(new IntervalRange(1, 2, 1));
    intervals.add(new IntervalRange(3, 4, 1));
    intervals.add(new IntervalRange(5, 6, 1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 1, 0),
        new IntervalRange(1, 2, 1),
        new IntervalRange(2, 3, 0),
        new IntervalRange(3, 4, 1),
        new IntervalRange(4, 5, 0),
        new IntervalRange(5, 6, 1),
        new IntervalRange(6, 7, 0),
    ]);

    intervals.clear();
    intervals.add(new IntervalRange(1, 2, 1));
    intervals.add(new IntervalRange(2, 3, 1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 1, 0),
        new IntervalRange(1, 3, 1),
        new IntervalRange(3, 7, 0),
    ]);

    intervals.clear();
    intervals.add(new IntervalRange(1, 4, 1));
    intervals.add(new IntervalRange(3, 5, 1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 1, 0),
        new IntervalRange(1, 3, 1),
        new IntervalRange(3, 4, 2),
        new IntervalRange(4, 5, 1),
        new IntervalRange(5, 7, 0),
    ]);

    intervals.clear();
    intervals.add(new IntervalRange(2, 3.5, 1));
    intervals.add(new IntervalRange(3, 5, 1));
    intervals.add(new IntervalRange(6, 7, 1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 2, 0),
        new IntervalRange(2, 3, 1),
        new IntervalRange(3, 3.5, 2),
        new IntervalRange(3.5, 5, 1),
        new IntervalRange(5, 6, 0),
        new IntervalRange(6, 7, 1),
    ]);

    intervals.clear();
    intervals.add(new IntervalRange(2, 5, 1));
    intervals.add(new IntervalRange(3, 3.5, -1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 2, 0),
        new IntervalRange(2, 3, 1),
        new IntervalRange(3, 3.5, 0),
        new IntervalRange(3.5, 5, 1),
        new IntervalRange(5, 7, 0),
    ]);

    intervals.clear();
    intervals.add(new IntervalRange(2, 5, 1));
    intervals.add(new IntervalRange(3, 3.5, -1));
    intervals.add(new IntervalRange(3, 3.5, 1));
    assertEq(intervals.intervalRanges(), [
        new IntervalRange(0, 2, 0),
        new IntervalRange(2, 5, 1),
        new IntervalRange(5, 7, 0),
    ]);
}
