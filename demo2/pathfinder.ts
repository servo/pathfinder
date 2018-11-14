// pathfinder/demo2/pathfinder.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import SVG from "../resources/svg/Ghostscript_Tiger.svg";

const SVGPath = require('svgpath');

const TILE_SIZE: number = 16.0;
const GLOBAL_OFFSET: Point2D = {x: 400.0, y: 200.0};

const SVG_NS: string = "http://www.w3.org/2000/svg";

type Point2D = {x: number, y: number};
type Size2D = {width: number, height: number};
type Rect = {origin: Point2D, size: Size2D};
type Vector3D = {x: number, y: number, z: number};

type Edge = 'left' | 'top' | 'right' | 'bottom';

type SVGPath = any;

class App {
    private svg: XMLDocument;

    constructor(svg: XMLDocument) {
        this.svg = svg;
    }

    run(): void {
        const svgElement = unwrapNull(this.svg.documentElement).cloneNode(true);
        document.body.appendChild(svgElement);

        const pathElements = Array.from(document.getElementsByTagName('path'));
        const tiles: Tile[] = [];

        for (let pathElementIndex = 0;
             pathElementIndex < 15;
             pathElementIndex++) {
            const pathElement = pathElements[pathElementIndex];

            const path = canonicalizePath(SVGPath(unwrapNull(pathElement.getAttribute('d'))));
            const boundingRect = this.boundingRectOfPath(path);

            //console.log("path " + pathElementIndex, path.toString(), ":", boundingRect);

            let y = boundingRect.origin.y;
            while (true) {
                let x = boundingRect.origin.x;
                while (true) {
                    const tileBounds = {
                        origin: {x, y},
                        size: {width: TILE_SIZE, height: TILE_SIZE},
                    };
                    const tilePath = this.clipPathToRect(path, tileBounds);

                    tiles.push(new Tile(pathElementIndex, tilePath, tileBounds.origin));

                    if (x >= boundingRect.origin.x + boundingRect.size.width)
                        break;
                    x += TILE_SIZE;
                }

                if (y >= boundingRect.origin.y + boundingRect.size.height)
                    break;
                y += TILE_SIZE;
            }

            for (const tile of tiles) {
                const newSVG = staticCast(document.createElementNS(SVG_NS, 'svg'), SVGElement);
                newSVG.setAttribute('class', "tile");
                newSVG.style.left = (GLOBAL_OFFSET.x + tile.origin.x) + "px";
                newSVG.style.top = (GLOBAL_OFFSET.y + tile.origin.y) + "px";
                newSVG.style.width = TILE_SIZE + "px";
                newSVG.style.height = TILE_SIZE + "px";

                const newPath = document.createElementNS(SVG_NS, 'path');
                newPath.setAttribute('d',
                                    tile.path
                                        .translate(-tile.origin.x, -tile.origin.y)
                                        .toString());

                let color = "#";
                for (let i = 0; i < 6; i++)
                    color += Math.floor(Math.random() * 16).toString(16);
                newPath.setAttribute('fill', color);

                newSVG.appendChild(newPath);
                document.body.appendChild(newSVG);
            }
        }

        document.body.removeChild(svgElement);
    }

    private clipPathToRect(path: SVGPath, tileBounds: Rect): SVGPath {
        path = this.clipPathToEdge('left', tileBounds.origin.x, path);
        path = this.clipPathToEdge('top', tileBounds.origin.y, path);
        path = this.clipPathToEdge('right', tileBounds.origin.x + tileBounds.size.width, path);
        path = this.clipPathToEdge('bottom', tileBounds.origin.y + tileBounds.size.height, path);
        return path;
    }

    private clipPathToEdge(edge: Edge, edgePos: number, input: SVGPath): SVGPath {
        let pathStart: Point2D | null = null, from = {x: 0, y: 0}, firstPoint = false;
        let output: string[][] = [];
        input.iterate((segment: string[], index: number, x: number, y: number) => {
            const event = segment[0];
            let to;
            switch (event) {
            case 'M':
                from = {
                    x: parseFloat(segment[segment.length - 2]),
                    y: parseFloat(segment[segment.length - 1]),
                };
                pathStart = from;
                firstPoint = true;
                return;
            case 'Z':
                if (pathStart == null)
                    return;
                to = pathStart;
                break;
            default:
                to = {
                    x: parseFloat(segment[segment.length - 2]),
                    y: parseFloat(segment[segment.length - 1]),
                };
                break;
            }

            if (this.pointIsInside(edge, edgePos, to)) {
                if (!this.pointIsInside(edge, edgePos, from)) {
                    this.addLine(this.computeLineIntersection(edge, edgePos, from, to),
                                 output,
                                 firstPoint);
                    firstPoint = false;
                }
                this.addLine(to, output, firstPoint);
                firstPoint = false;
            } else if (this.pointIsInside(edge, edgePos, from)) {
                this.addLine(this.computeLineIntersection(edge, edgePos, from, to),
                             output,
                             firstPoint);
                firstPoint = false;
            }

            from = to;

            if (event === 'Z') {
                output.push(['Z']);
                pathStart = null;
            }
        });

        return SVGPath(output.map(segment => segment.join(" ")).join(" "));
    }

    private addLine(to: Point2D, output: string[][], firstPoint: boolean) {
        if (firstPoint)
            output.push(['M', "" + to.x, "" + to.y]);
        else
            output.push(['L', "" + to.x, "" + to.y]);
    }

    private pointIsInside(edge: Edge, edgePos: number, point: Point2D): boolean {
        switch (edge) {
        case 'left':    return point.x >= edgePos;
        case 'top':     return point.y >= edgePos;
        case 'right':   return point.x <= edgePos;
        case 'bottom':  return point.y <= edgePos;
        }
    }

    private computeLineIntersection(edge: Edge,
                                    edgePos: number,
                                    startPoint: Point2D,
                                    endpoint: Point2D):
                                    Point2D {
        const start = {x: startPoint.x, y: startPoint.y, z: 1.0};
        const end = {x: endpoint.x, y: endpoint.y, z: 1.0};

        let edgeVector: Vector3D;
        switch (edge) {
        case 'left':
        case 'right':
            edgeVector = {x: 1.0, y: 0.0, z: -edgePos};
            break;
        default:
            edgeVector = {x: 0.0, y: 1.0, z: -edgePos};
            break;
        }

        const intersection = this.cross(this.cross(start, end), edgeVector);
        return {x: intersection.x / intersection.z, y: intersection.y / intersection.z};
    }

    private boundingRectOfPath(path: SVGPath): Rect {
        let minX: number | null = null, minY: number | null = null;
        let maxX: number | null = null, maxY: number | null = null;
        path.iterate((segment: string[], index: number, x: number, y: number) => {
            for (let i = 1; i < segment.length; i += 2) {
                const x = parseFloat(segment[i]), y = parseFloat(segment[i + 1]);
                minX = minX == null ? x : Math.min(minX, x);
                minY = minY == null ? y : Math.min(minY, y);
                maxX = maxX == null ? x : Math.max(maxX, x);
                maxY = maxY == null ? y : Math.max(maxY, y);
                //console.log("x", x, "y", y, "maxX", maxX, "maxY", maxY, "segment", segment);
            }
        });
        if (minX == null || minY == null || maxX == null || maxY == null)
            return {origin: {x: 0, y: 0}, size: {width: 0, height: 0}};
        return {origin: {x: minX, y: minY}, size: {width: maxX - minX, height: maxY - minY}};
    }

    private cross(a: Vector3D, b: Vector3D): Vector3D {
        return {
            x: a.y*b.z - a.z*b.y,
            y: a.z*b.x - a.x*b.z,
            z: a.x*b.y - a.y*b.x,
        };
    }
}

class Tile {
    pathIndex: number;
    path: SVGPath;
    origin: Point2D;

    constructor(pathIndex: number, path: SVGPath, origin: Point2D) {
        this.pathIndex = pathIndex;
        this.path = path;
        this.origin = origin;
    }
}

function canonicalizePath(path: SVGPath): SVGPath {
    return path.abs().iterate((segment: string[], index: number, x: number, y: number) => {
        if (segment[0] === 'H')
            return [['L', segment[1], '0']];
        if (segment[0] === 'V')
            return [['L', '0', segment[1]]];
        return [segment];
    });
}

function main(): void {
    window.fetch(SVG).then(svg => {
        svg.text().then(svgText => {
            const svg = staticCast((new DOMParser).parseFromString(svgText, 'image/svg+xml'),
                                   XMLDocument);
            new App(svg).run();
        });
    });
}

document.addEventListener('DOMContentLoaded', () => main(), false);

function staticCast<T>(value: any, constructor: { new(...args: any[]): T }): T {
    if (!(value instanceof constructor))
        throw new Error("Invalid dynamic cast");
    return value;
}

function unwrapNull<T>(value: T | null): T {
    if (value == null)
        throw new Error("Unexpected null");
    return value;
}
