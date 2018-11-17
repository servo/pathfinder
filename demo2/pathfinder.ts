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
import AREA_LUT from "../resources/textures/area-lut.png";

const SVGPath: (path: string) => SVGPath = require('svgpath');
const parseColor: (color: string) => any = require('parse-color');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const TILE_SIZE: number = 32.0;
const STENCIL_FRAMEBUFFER_SIZE: number = TILE_SIZE * 128;

const QUAD_VERTEX_POSITIONS: Uint8Array = new Uint8Array([
    0, 0,
    1, 0,
    1, 1,
    0, 1,
]);

const EPSILON: number = 1e-6;

interface SVGPath {
    abs(): SVGPath;
    translate(x: number, y: number): SVGPath;
    matrix(m: number[]): SVGPath;
    iterate(f: (segment: string[], index: number, x: number, y: number) => string[][] | void):
            SVGPath;
}

class Point2D {
    x: number;
    y: number;

    constructor(x: number, y: number) {
        this.x = x;
        this.y = y;
    }

    approxEq(other: Point2D): boolean {
        return Math.abs(this.x - other.x) <= EPSILON && Math.abs(this.y - other.y) <= EPSILON;
    }
}

interface Size2D {
    width: number;
    height: number;
}

interface Rect {
    origin: Point2D;
    size: Size2D;
}

interface Vector3D {
    x: number;
    y: number;
    z: number;
}

class Matrix2D {
    a: number; b: number;
    c: number; d: number;
    tx: number; ty: number;

    constructor(a: number, b: number, c: number, d: number, tx: number, ty: number) {
        this.a = a; this.b = b;
        this.c = c; this.d = d;
        this.tx = tx; this.ty = ty;
    }
}

const GLOBAL_TRANSFORM: Matrix2D = new Matrix2D(3.0, 0.0, 0.0, 3.0, 800.0, 550.0);

interface Color {
    r: number;
    g: number;
    b: number;
    a: number;
}

type Edge = 'left' | 'top' | 'right' | 'bottom';

class App {
    private canvas: HTMLCanvasElement;
    private svg: XMLDocument;
    private areaLUT: HTMLImageElement;

    private gl: WebGL2RenderingContext;
    private disjointTimerQueryExt: any;
    private areaLUTTexture: WebGLTexture;
    private stencilTexture: WebGLTexture;
    private stencilFramebuffer: WebGLFramebuffer;
    private stencilProgram: Program<'FramebufferSize' | 'TileSize' | 'AreaLUT',
                                    'TessCoord' | 'From' | 'To' | 'TileIndex'>;
    private coverProgram:
        Program<'FramebufferSize' | 'TileSize' | 'StencilTexture' | 'StencilTextureSize',
                'TessCoord' | 'TileOrigin' | 'TileIndex' | 'Color'>;
    private quadVertexBuffer: WebGLBuffer;
    private stencilVertexPositionsBuffer: WebGLBuffer;
    private stencilVertexTileIndicesBuffer: WebGLBuffer;
    private stencilVertexArray: WebGLVertexArrayObject;
    private coverVertexBuffer: WebGLBuffer;
    private coverVertexArray: WebGLVertexArrayObject;

    private scene: Scene | null;
    private primitiveCount: number | null;

    constructor(svg: XMLDocument, areaLUT: HTMLImageElement) {
        const canvas = staticCast(document.getElementById('canvas'), HTMLCanvasElement);
        this.canvas = canvas;
        this.svg = svg;
        this.areaLUT = areaLUT;

        const devicePixelRatio = window.devicePixelRatio;
        canvas.width = window.innerWidth * devicePixelRatio;
        canvas.height = window.innerHeight * devicePixelRatio;
        canvas.style.width = window.innerWidth + "px";
        canvas.style.height = window.innerHeight + "px";

        const gl = unwrapNull(this.canvas.getContext('webgl2', {antialias: false}));
        this.gl = gl;
        gl.getExtension('EXT_color_buffer_float');
        this.disjointTimerQueryExt = gl.getExtension('EXT_disjoint_timer_query');

        this.areaLUTTexture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, this.areaLUTTexture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, areaLUT);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

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
                                         ['TessCoord', 'TileOrigin', 'TileIndex', 'Color']);
        this.coverProgram = coverProgram;

        const stencilProgram = new Program(gl,
                                           STENCIL_VERTEX_SHADER_SOURCE,
                                           STENCIL_FRAGMENT_SHADER_SOURCE,
                                           ['FramebufferSize', 'TileSize', 'AreaLUT'],
                                           ['TessCoord', 'From', 'To', 'TileIndex']);
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
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.vertexAttribPointer(stencilProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexPositionsBuffer);
        gl.vertexAttribPointer(stencilProgram.attributes.From, 2, gl.FLOAT, false, 16, 0);
        gl.vertexAttribDivisor(stencilProgram.attributes.From, 1);
        gl.vertexAttribPointer(stencilProgram.attributes.To, 2, gl.FLOAT, false, 16, 8);
        gl.vertexAttribDivisor(stencilProgram.attributes.To, 1);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexTileIndicesBuffer);
        gl.vertexAttribIPointer(stencilProgram.attributes.TileIndex,
                                1,
                                gl.UNSIGNED_SHORT,
                                0,
                                0);
        gl.vertexAttribDivisor(stencilProgram.attributes.TileIndex, 1);
        gl.enableVertexAttribArray(stencilProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(stencilProgram.attributes.From);
        gl.enableVertexAttribArray(stencilProgram.attributes.To);
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
        gl.vertexAttribPointer(coverProgram.attributes.TileOrigin, 2, gl.SHORT, false, 10, 0);
        gl.vertexAttribDivisor(coverProgram.attributes.TileOrigin, 1);
        gl.vertexAttribIPointer(coverProgram.attributes.TileIndex, 1, gl.UNSIGNED_SHORT, 10, 4);
        gl.vertexAttribDivisor(coverProgram.attributes.TileIndex, 1);
        gl.vertexAttribPointer(coverProgram.attributes.Color, 4, gl.UNSIGNED_BYTE, true, 10, 6);
        gl.vertexAttribDivisor(coverProgram.attributes.Color, 1);
        gl.enableVertexAttribArray(coverProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(coverProgram.attributes.TileOrigin);
        gl.enableVertexAttribArray(coverProgram.attributes.TileIndex);
        gl.enableVertexAttribArray(coverProgram.attributes.Color);

        // Set up event handlers.
        this.canvas.addEventListener('click', event => this.onClick(event), false);

        this.scene = null;
        this.primitiveCount = 0;
    }

    redraw(): void {
        const gl = this.gl, canvas = this.canvas, scene = unwrapNull(this.scene);

        // Start timer.
        let timerQuery = null;
        if (this.disjointTimerQueryExt != null) {
            timerQuery = unwrapNull(gl.createQuery());
            gl.beginQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT, timerQuery);
        }

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
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.areaLUTTexture);
        gl.uniform1i(this.stencilProgram.uniforms.AreaLUT, 0);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, unwrapNull(this.primitiveCount));
        gl.disable(gl.BLEND);

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
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, scene.tiles.length);
        gl.disable(gl.BLEND);

        // End timer.
        if (timerQuery != null) {
            gl.endQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT);
            waitForQuery(gl, this.disjointTimerQueryExt, timerQuery);
        }
    }

    buildScene(): void {
        this.scene = new Scene(this.svg);
        console.log(this.scene.tiles.length, "tiles");
    }

    prepare(): void {
        const gl = this.gl, scene = unwrapNull(this.scene);

        // Construct stencil VBOs.
        let primitiveCount = 0;
        const stencilVertexPositions: number[] = [], stencilVertexTileIndices: number[] = [];
        const primitiveCountHistogram: number[] = [];
        for (let tileIndex = 0; tileIndex < scene.tiles.length; tileIndex++) {
            const tile = scene.tiles[tileIndex];
            let firstPoint = {x: 0.0, y: 0.0}, lastPoint = {x: 0.0, y: 0.0};
            let primitiveCountForThisTile = 0;
            tile.path.iterate(segment => {
                /*if (primitiveCountForThisTile > 0)
                    return;*/

                let point;
                if (segment[0] === 'Z') {
                    point = firstPoint;
                } else {
                    point = {
                        x: parseFloat(segment[segment.length - 2]),
                        y: parseFloat(segment[segment.length - 1]),
                    };
                }

                /*
                if (!(point.x > -1.0))
                    throw new Error("x too low");
                if (!(point.y > -1.0))
                    throw new Error("y too low");
                if (!(point.x < TILE_SIZE + 1.0))
                    throw new Error("x too high:" + point.x);
                if (!(point.y < TILE_SIZE + 1.0))
                    throw new Error("y too high");
                    */

                if (segment[0] === 'M') {
                    firstPoint = point;
                } else {
                    stencilVertexPositions.push(lastPoint.x, lastPoint.y, point.x, point.y);
                    stencilVertexTileIndices.push(tileIndex);
                    primitiveCount++;
                    primitiveCountForThisTile++;
                }
                lastPoint = point;
            });

            if (primitiveCountHistogram[primitiveCountForThisTile] == null)
                primitiveCountHistogram[primitiveCountForThisTile] = 0;
            primitiveCountHistogram[primitiveCountForThisTile]++;
        }
        console.log(stencilVertexPositions);
        console.log("histogram", primitiveCountHistogram);

        // Populate the stencil VBOs.
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(stencilVertexPositions), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexTileIndicesBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Uint16Array(stencilVertexTileIndices), gl.STATIC_DRAW);

        // Populate the cover VBO.
        const coverVertexBufferData = new Int16Array(scene.tiles.length * 5);
        for (let tileIndex = 0; tileIndex < scene.tiles.length; tileIndex++) {
            const tile = scene.tiles[tileIndex];
            const color = scene.pathColors[tile.pathIndex];
            coverVertexBufferData[tileIndex * 5 + 0] = Math.floor(tile.origin.x);
            coverVertexBufferData[tileIndex * 5 + 1] = Math.floor(tile.origin.y);
            coverVertexBufferData[tileIndex * 5 + 2] = tileIndex;
            coverVertexBufferData[tileIndex * 5 + 3] = color.r | (color.g << 8);
            coverVertexBufferData[tileIndex * 5 + 4] = color.b | (color.a << 8);
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, this.coverVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, coverVertexBufferData, gl.DYNAMIC_DRAW);
        console.log(coverVertexBufferData);

        this.primitiveCount = primitiveCount;
        console.log(primitiveCount + " primitives");
    }

    private onClick(event: MouseEvent): void {
        this.redraw();
    }
}

class Scene {
    tiles: Tile[];
    pathColors: Color[];

    constructor(svg: XMLDocument) {
        const svgElement = unwrapNull(svg.documentElement).cloneNode(true);
        document.body.appendChild(svgElement);

        const pathElements = Array.from(document.getElementsByTagName('path'));
        const tiles: Tile[] = [], pathColors = [];

        for (let pathElementIndex = 0;
             pathElementIndex < pathElements.length;
             pathElementIndex++) {
            const pathElement = pathElements[pathElementIndex];

            const style = window.getComputedStyle(pathElement);
            let paint: string;
            if (style.fill != null && style.fill !== 'none') {
                paint = style.fill;
            /*} else if (style.stroke != null && style.stroke !== 'none') {
                paint = style.stroke;*/
            } else {
                pathColors.push({r: 0, g: 0, b: 0, a: 0});
                continue;
            }
            const color = parseColor(paint).rgba;
            pathColors.push({
                r: color[0],
                g: color[1],
                b: color[2],
                a: Math.round(color[3] * 255.),
            });

            let path = SVGPath(unwrapNull(pathElement.getAttribute('d')));
            path = path.matrix([
                GLOBAL_TRANSFORM.a, GLOBAL_TRANSFORM.b,
                GLOBAL_TRANSFORM.c, GLOBAL_TRANSFORM.d,
                GLOBAL_TRANSFORM.tx, GLOBAL_TRANSFORM.ty,
            ]);

            path = flattenPath(path);
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
                        origin: new Point2D(x, y),
                        size: {width: TILE_SIZE, height: TILE_SIZE},
                    };
                    const tilePath = this.clipPathToRect(path, tileBounds);

                    if (tilePath.toString().length > 0) {
                        tilePath.translate(-tileBounds.origin.x, -tileBounds.origin.y);
                        if (!pathIsSquare(tilePath, TILE_SIZE))
                            tiles.push(new Tile(pathElementIndex, tilePath, tileBounds.origin));
                    }

                    if (x >= boundingRect.origin.x + boundingRect.size.width)
                        break;
                    x += TILE_SIZE;
                }

                if (y >= boundingRect.origin.y + boundingRect.size.height)
                    break;
                y += TILE_SIZE;
            }
        }

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

            const color = pathColors[tile.pathIndex];
            newPath.setAttribute('fill',
                                 "rgba(" + color.r + "," + color.g + "," + color.b + "," +
                                 (color.a / 255.0));

            newSVG.appendChild(newPath);
            document.body.appendChild(newSVG);
        }*/

        document.body.removeChild(svgElement);

        console.log(tiles);
        this.tiles = tiles;
        this.pathColors = pathColors;
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
            return {origin: new Point2D(0, 0), size: {width: 0, height: 0}};
        return {origin: new Point2D(minX, minY), size: {width: maxX - minX, height: maxY - minY}};
    }

    private clipPathToRect(path: SVGPath, tileBounds: Rect): SVGPath {
        path = this.clipPathToEdge('left', tileBounds.origin.x, path);
        path = this.clipPathToEdge('top', tileBounds.origin.y, path);
        path = this.clipPathToEdge('right', tileBounds.origin.x + tileBounds.size.width, path);
        path = this.clipPathToEdge('bottom', tileBounds.origin.y + tileBounds.size.height, path);
        return path;
    }

    private clipPathToEdge(edge: Edge, edgePos: number, input: SVGPath): SVGPath {
        let pathStart: Point2D | null = null, from = new Point2D(0, 0), firstPoint = false;
        let output: string[][] = [];
        input.iterate(segment => {
            const event = segment[0];
            let to;
            switch (event) {
            case 'M':
                from = new Point2D(parseFloat(segment[segment.length - 2]),
                                   parseFloat(segment[segment.length - 1]));
                pathStart = from;
                firstPoint = true;
                return;
            case 'Z':
                if (pathStart == null)
                    return;
                to = pathStart;
                break;
            default:
                to = new Point2D(parseFloat(segment[segment.length - 2]),
                                 parseFloat(segment[segment.length - 1]));
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

        const intersection = cross(cross(start, end), edgeVector);
        return new Point2D(intersection.x / intersection.z, intersection.y / intersection.z);
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
    uniforms: {[key in U]: WebGLUniformLocation | null};
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

        const uniforms: {[key in U]?: WebGLUniformLocation | null} = {};
        for (const uniformName of uniformNames)
            uniforms[uniformName] = gl.getUniformLocation(this.program, "u" + uniformName);
        this.uniforms = uniforms as {[key in U]: WebGLUniformLocation | null};

        const attributes: {[key in A]?: number} = {};
        for (const attributeName of attributeNames) {
            attributes[attributeName] = unwrapNull(gl.getAttribLocation(this.program,
                                                                        "a" + attributeName));
        }
        this.attributes = attributes as {[key in A]: number};
    }
}

class PathSegment {
    command: string;
    points: Point2D[];

    constructor(segment: string[]) {
        const points = [];
        for (let i = 1; i < segment.length; i += 2)
            points.push(new Point2D(parseFloat(segment[i]), parseFloat(segment[i + 1])));
        this.points = points;
        this.command = segment[0];
    }
}

function flattenPath(path: SVGPath): SVGPath {
    return path.abs().iterate(segment => {
        if (segment[0] === 'Q')
            return [['L', segment[1], segment[2]], ['L', segment[3], segment[4]]];
        if (segment[0] === 'C') {
            return [
                ['L', segment[1], segment[2]],
                ['L', segment[3], segment[4]],
                ['L', segment[5], segment[6]],
            ];
        }
        return [segment];
    });
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

function cross(a: Vector3D, b: Vector3D): Vector3D {
    return {
        x: a.y*b.z - a.z*b.y,
        y: a.z*b.x - a.x*b.z,
        z: a.x*b.y - a.y*b.x,
    };
}

function waitForQuery(gl: WebGL2RenderingContext, disjointTimerQueryExt: any, query: WebGLQuery):
                      void {
    const queryResultAvailable = disjointTimerQueryExt.QUERY_RESULT_AVAILABLE_EXT;
    const queryResult = disjointTimerQueryExt.QUERY_RESULT_EXT;
    if (!disjointTimerQueryExt.getQueryObjectEXT(query, queryResultAvailable)) {
        setTimeout(() => waitForQuery(gl, disjointTimerQueryExt, query), 10);
        return;
    }
    const elapsed = disjointTimerQueryExt.getQueryObjectEXT(query, queryResult) / 1000000.0;
    console.log(elapsed + "ms elapsed");
}

function pathIsSquare(path: SVGPath, squareLength: number): boolean {
    const SQUARE_VERTICES = [
        new Point2D(0.0, 0.0),
        new Point2D(0.0, squareLength),
        new Point2D(squareLength, squareLength),
        new Point2D(squareLength, 0.0),
    ];
    let result = true;
    path.iterate((segment, index) => {
        if (index < SQUARE_VERTICES.length) {
            const point = new Point2D(parseFloat(segment[1]), parseFloat(segment[2]));
            result = result && point.approxEq(SQUARE_VERTICES[index]);
        } else if (index === SQUARE_VERTICES.length) {
            const point = new Point2D(parseFloat(segment[1]), parseFloat(segment[2]));
            result = result && (segment[0] === 'Z' || point.approxEq(SQUARE_VERTICES[0]));
        } else if (index === SQUARE_VERTICES.length + 1) {
            result = result && segment[0] === 'Z';
        } else {
            result = false;
        }
    });
    return result;
}

function main(): void {
    window.fetch(SVG).then(svg => {
        svg.text().then(svgText => {
            const svg = staticCast((new DOMParser).parseFromString(svgText, 'image/svg+xml'),
                                   XMLDocument);
            const image = new Image;
            image.src = AREA_LUT;
            image.addEventListener('load', event => {
                try {
                    const app = new App(svg, image);
                    app.buildScene();
                    app.prepare();
                    app.redraw();
                } catch (e) {
                    console.error("error", e, e.stack);
                }
            }, false);
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
