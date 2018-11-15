// pathfinder/demo2/pathfinder.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import COVER_VERTEX_SHADER_SOURCE from "./cover.vs.glsl";
import COVER_FRAGMENT_SHADER_SOURCE from "./cover.fs.glsl";
import STENCIL_VERTEX_SHADER_SOURCE from "./stencil.vs.glsl";
import STENCIL_FRAGMENT_SHADER_SOURCE from "./stencil.fs.glsl";
import SVG from "../resources/svg/Ghostscript_Tiger.svg";

const SVGPath: (path: string) => SVGPath = require('svgpath');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const TILE_SIZE: number = 16.0;
const STENCIL_FRAMEBUFFER_SIZE: number = TILE_SIZE * 256;

const GLOBAL_OFFSET: Point2D = {x: 200.0, y: 150.0};

const QUAD_VERTEX_POSITIONS: Uint8Array = new Uint8Array([
    0, 0,
    1, 0,
    0, 1,
    1, 1,
]);

interface SVGPath {
    abs(): SVGPath;
    translate(x: number, y: number): SVGPath;
    iterate(f: (segment: string[], index: number, x: number, y: number) => string[][] | void):
            SVGPath;
}

type Point2D = {x: number, y: number};
type Size2D = {width: number, height: number};
type Rect = {origin: Point2D, size: Size2D};
type Vector3D = {x: number, y: number, z: number};

type Edge = 'left' | 'top' | 'right' | 'bottom';

class App {
    private canvas: HTMLCanvasElement;
    private svg: XMLDocument;

    private gl: WebGL2RenderingContext;
    private stencilTexture: WebGLTexture;
    private stencilFramebuffer: WebGLFramebuffer;
    private coverProgram:
        Program<'FramebufferSize' | 'TileSize' | 'StencilTexture' | 'StencilTextureSize',
                'TessCoord' | 'TileOrigin' | 'TileIndex'>;
    private stencilProgram: Program<'FramebufferSize' | 'TileSize', 'Position' | 'TileIndex'>;
    private quadVertexBuffer: WebGLBuffer;
    private stencilVertexPositionsBuffer: WebGLBuffer;
    private stencilVertexTileIndicesBuffer: WebGLBuffer;
    private stencilVertexArray: WebGLVertexArrayObject;
    private coverVertexBuffer: WebGLBuffer;
    private coverVertexArray: WebGLVertexArrayObject;

    constructor(svg: XMLDocument) {
        this.canvas = staticCast(document.getElementById('canvas'), HTMLCanvasElement);
        this.svg = svg;

        const gl = unwrapNull(this.canvas.getContext('webgl2', {antialias: false}));
        this.gl = gl;
        gl.getExtension('EXT_color_buffer_float');

        this.stencilTexture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, this.stencilTexture);
        gl.texImage2D(gl.TEXTURE_2D,
                      0,
                      gl.R16F,
                      STENCIL_FRAMEBUFFER_SIZE,
                      STENCIL_FRAMEBUFFER_SIZE,
                      0,
                      gl.RED,
                      gl.HALF_FLOAT,
                      null);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

        this.stencilFramebuffer = unwrapNull(gl.createFramebuffer());
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.stencilFramebuffer);
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                gl.COLOR_ATTACHMENT0,
                                gl.TEXTURE_2D,
                                this.stencilTexture,
                                0);
        if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) != gl.FRAMEBUFFER_COMPLETE)
            throw new Error("Stencil framebuffer incomplete!");

        const coverProgram = new Program(gl,
                                         COVER_VERTEX_SHADER_SOURCE,
                                         COVER_FRAGMENT_SHADER_SOURCE,
                                         [
                                             'FramebufferSize',
                                             'TileSize',
                                             'StencilTexture',
                                             'StencilTextureSize'
                                         ],
                                         ['TessCoord', 'TileOrigin', 'TileIndex']);
        this.coverProgram = coverProgram;

        const stencilProgram = new Program(gl,
                                           STENCIL_VERTEX_SHADER_SOURCE,
                                           STENCIL_FRAGMENT_SHADER_SOURCE,
                                           ['FramebufferSize', 'TileSize'],
                                           ['Position', 'TileIndex']);
        this.stencilProgram = stencilProgram;

        // Initialize quad VBO.
        this.quadVertexBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, QUAD_VERTEX_POSITIONS, gl.STATIC_DRAW);

        // Initialize stencil VBOs.
        this.stencilVertexPositionsBuffer = unwrapNull(gl.createBuffer());
        this.stencilVertexTileIndicesBuffer = unwrapNull(gl.createBuffer());

        // Initialize stencil VAO.
        this.stencilVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.stencilVertexArray);
        gl.useProgram(this.stencilProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexPositionsBuffer);
        gl.vertexAttribPointer(stencilProgram.attributes.Position, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexTileIndicesBuffer);
        gl.vertexAttribIPointer(stencilProgram.attributes.TileIndex,
                                1,
                                gl.UNSIGNED_SHORT,
                                0,
                                0);
        gl.enableVertexAttribArray(stencilProgram.attributes.Position);
        gl.enableVertexAttribArray(stencilProgram.attributes.TileIndex);

        // Initialize cover VBO.
        this.coverVertexBuffer = unwrapNull(gl.createBuffer());

        // Initialize cover VAO.
        this.coverVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.coverVertexArray);
        gl.useProgram(this.coverProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.vertexAttribPointer(coverProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.coverVertexBuffer);
        gl.vertexAttribPointer(coverProgram.attributes.TileOrigin, 2, gl.SHORT, false, 6, 0);
        gl.vertexAttribDivisor(coverProgram.attributes.TileOrigin, 1);
        gl.vertexAttribIPointer(coverProgram.attributes.TileIndex, 1, gl.UNSIGNED_SHORT, 6, 4);
        gl.vertexAttribDivisor(coverProgram.attributes.TileIndex, 1);
        gl.enableVertexAttribArray(coverProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(coverProgram.attributes.TileOrigin);
        gl.enableVertexAttribArray(coverProgram.attributes.TileIndex);

        // TODO(pcwalton)
    }

    run(): void {
        const gl = this.gl, canvas = this.canvas;

        const tiles = this.createTiles();
        console.log(tiles.length, "tiles");

        // Construct stencil VBOs.
        let primitives = 0;
        const stencilVertexPositions: number[] = [], stencilVertexTileIndices: number[] = [];
        for (let tileIndex = 0; tileIndex < tiles.length; tileIndex++) {
            const tile = tiles[tileIndex];
            let lastPoint = {x: 0.0, y: 0.0};
            tile.path.iterate(segment => {
                if (segment[0] === 'Z')
                    return;
                const point = {
                    x: parseFloat(segment[segment.length - 2]) - tile.origin.x,
                    y: parseFloat(segment[segment.length - 1]) - tile.origin.y,
                };
                if (!(point.x > -1.0))
                    throw new Error("x too low");
                if (!(point.y > -1.0))
                    throw new Error("y too low");
                if (!(point.x < TILE_SIZE + 1.0))
                    throw new Error("x too high:" + point.x);
                if (!(point.y < TILE_SIZE + 1.0))
                    throw new Error("y too high");
                if (segment[0] !== 'M') {
                    stencilVertexPositions.push(lastPoint.x, lastPoint.y, point.x, point.y);
                    stencilVertexTileIndices.push(tileIndex, tileIndex);
                    primitives++;
                }
                lastPoint = point;
            });
        }
        console.log(stencilVertexPositions);

        // Populate the stencil VBOs.
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(stencilVertexPositions), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexTileIndicesBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Uint16Array(stencilVertexTileIndices), gl.STATIC_DRAW);

        // Stencil.
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.stencilFramebuffer);
        gl.viewport(0, 0, STENCIL_FRAMEBUFFER_SIZE, STENCIL_FRAMEBUFFER_SIZE);
        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        gl.bindVertexArray(this.stencilVertexArray);
        gl.useProgram(this.stencilProgram.program);
        gl.uniform2f(this.stencilProgram.uniforms.FramebufferSize,
                     STENCIL_FRAMEBUFFER_SIZE,
                     STENCIL_FRAMEBUFFER_SIZE);
        gl.uniform2f(this.stencilProgram.uniforms.TileSize, TILE_SIZE, TILE_SIZE);
        gl.drawArrays(gl.LINES, 0, primitives * 2);

        // Populate the cover VBO.
        const coverVertexBufferData = new Int16Array(tiles.length * 3);
        for (let tileIndex = 0; tileIndex < tiles.length; tileIndex++) {
            coverVertexBufferData[tileIndex * 3 + 0] = Math.floor(tiles[tileIndex].origin.x);
            coverVertexBufferData[tileIndex * 3 + 1] = Math.floor(tiles[tileIndex].origin.y);
            coverVertexBufferData[tileIndex * 3 + 2] = tileIndex;
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, this.coverVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, coverVertexBufferData, gl.DYNAMIC_DRAW);
        console.log(coverVertexBufferData);

        // Cover.
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        const framebufferSize = {width: canvas.width, height: canvas.height};
        gl.viewport(0, 0, framebufferSize.width, framebufferSize.height);
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        gl.bindVertexArray(this.coverVertexArray);
        gl.useProgram(this.coverProgram.program);
        gl.uniform2f(this.coverProgram.uniforms.FramebufferSize,
                     framebufferSize.width,
                     framebufferSize.height);
        gl.uniform2f(this.coverProgram.uniforms.TileSize, TILE_SIZE, TILE_SIZE);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.stencilTexture);
        gl.uniform1i(this.coverProgram.uniforms.StencilTexture, 0);
        gl.uniform2f(this.coverProgram.uniforms.StencilTextureSize,
                     STENCIL_FRAMEBUFFER_SIZE,
                     STENCIL_FRAMEBUFFER_SIZE);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
        gl.enable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, tiles.length);
        gl.disable(gl.BLEND);

        /*
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
        */
    }

    private createTiles(): Tile[] {
        const svgElement = unwrapNull(this.svg.documentElement).cloneNode(true);
        document.body.appendChild(svgElement);

        const pathElements = Array.from(document.getElementsByTagName('path'));
        const tiles: Tile[] = [];

        for (let pathElementIndex = 0;
             pathElementIndex < pathElements.length;
             pathElementIndex++) {
            const pathElement = pathElements[pathElementIndex];

            let path =
                SVGPath(unwrapNull(pathElement.getAttribute('d'))).translate(GLOBAL_OFFSET.x,
                                                                             GLOBAL_OFFSET.y);
            path = canonicalizePath(path);
            const boundingRect = this.boundingRectOfPath(path);

            /*console.log("path " + pathElementIndex, path.toString(), ":",
                        boundingRect.origin.x,
                        boundingRect.origin.y,
                        boundingRect.size.width,
                        boundingRect.size.height);*/

            let y = boundingRect.origin.y - boundingRect.origin.y % TILE_SIZE;
            while (true) {
                let x = boundingRect.origin.x - boundingRect.origin.x % TILE_SIZE;
                while (true) {
                    const tileBounds = {
                        origin: {x, y},
                        size: {width: TILE_SIZE, height: TILE_SIZE},
                    };
                    const tilePath = this.clipPathToRect(path, tileBounds);

                    if (tilePath.toString().length > 0)
                        tiles.push(new Tile(pathElementIndex, tilePath, tileBounds.origin));

                    if (x >= boundingRect.origin.x + boundingRect.size.width)
                        break;
                    x += TILE_SIZE;
                }

                if (y >= boundingRect.origin.y + boundingRect.size.height)
                    break;
                y += TILE_SIZE;
            }
        }

        document.body.removeChild(svgElement);

        return tiles;
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
        input.iterate(segment => {
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
        path.iterate(segment => {
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

class Program<U extends string, A extends string> {
    program: WebGLProgram;
    uniforms: {[key in U]: WebGLUniformLocation};
    attributes: {[key in A]: number};

    private vertexShader: WebGLShader;
    private fragmentShader: WebGLShader;

    constructor(gl: WebGL2RenderingContext,
                vertexShaderSource: string,
                fragmentShaderSource: string,
                uniformNames: U[],
                attributeNames: A[]) {
        this.vertexShader = unwrapNull(gl.createShader(gl.VERTEX_SHADER));
        gl.shaderSource(this.vertexShader, vertexShaderSource);
        gl.compileShader(this.vertexShader);
        if (!gl.getShaderParameter(this.vertexShader, gl.COMPILE_STATUS)) {
            console.error(gl.getShaderInfoLog(this.vertexShader));
            throw new Error("Vertex shader compilation failed!");
        }

        this.fragmentShader = unwrapNull(gl.createShader(gl.FRAGMENT_SHADER));
        gl.shaderSource(this.fragmentShader, fragmentShaderSource);
        gl.compileShader(this.fragmentShader);
        if (!gl.getShaderParameter(this.fragmentShader, gl.COMPILE_STATUS)) {
            console.error(gl.getShaderInfoLog(this.fragmentShader));
            throw new Error("Fragment shader compilation failed!");
        }

        this.program = unwrapNull(gl.createProgram());
        gl.attachShader(this.program, this.vertexShader);
        gl.attachShader(this.program, this.fragmentShader);
        gl.linkProgram(this.program);
        if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
            console.error(gl.getProgramInfoLog(this.program));
            throw new Error("Program linking failed!");
        }

        const uniforms: {[key in U]?: WebGLUniformLocation} = {};
        for (const uniformName of uniformNames) {
            uniforms[uniformName] = unwrapNull(gl.getUniformLocation(this.program,
                                                                     "u" + uniformName));
        }
        this.uniforms = uniforms as {[key in U]: WebGLUniformLocation};

        const attributes: {[key in A]?: number} = {};
        for (const attributeName of attributeNames) {
            attributes[attributeName] = unwrapNull(gl.getAttribLocation(this.program,
                                                                        "a" + attributeName));
        }
        this.attributes = attributes as {[key in A]: number};
    }
}

function canonicalizePath(path: SVGPath): SVGPath {
    return path.abs().iterate(segment => {
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
            try {
                new App(svg).run();
            } catch (e) {
                console.error("error", e, e.stack);
            }
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
