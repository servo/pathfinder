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
import OPAQUE_VERTEX_SHADER_SOURCE from "./opaque.vs.glsl";
import OPAQUE_FRAGMENT_SHADER_SOURCE from "./opaque.fs.glsl";
import STENCIL_VERTEX_SHADER_SOURCE from "./stencil.vs.glsl";
import STENCIL_FRAGMENT_SHADER_SOURCE from "./stencil.fs.glsl";
import SVG from "../resources/svg/Ghostscript_Tiger.svg";
import AREA_LUT from "../resources/textures/area-lut.png";
import {Matrix2D, Point2D, Rect, Size2D, Vector3D, approxEq, cross, lerp} from "./geometry";
import {flattenPath, Outline, makePathMonotonic} from "./path-utils";
import {SVGPath, TILE_SIZE, TileDebugger, Tiler, testIntervals, TileStrip} from "./tiling";
import {staticCast, unwrapNull} from "./util";

const SVGPath: (path: string) => SVGPath = require('svgpath');
const parseColor: (color: string) => any = require('parse-color');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const STENCIL_FRAMEBUFFER_SIZE: Size2D = {
    width: TILE_SIZE.width * 128,
    height: TILE_SIZE.height * 256,
};

const QUAD_VERTEX_POSITIONS: Uint8Array = new Uint8Array([
    0, 0,
    1, 0,
    1, 1,
    0, 1,
]);

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
                                    'TessCoord' | 'From' | 'Ctrl' | 'To' | 'TileIndex'>;
    private opaqueProgram: Program<'FramebufferSize' | 'TileSize',
                                   'TessCoord' | 'TileOrigin' | 'Color'>;
    private coverProgram:
        Program<'FramebufferSize' | 'TileSize' | 'StencilTexture' | 'StencilTextureSize',
                'TessCoord' | 'TileOrigin' | 'TileIndex' | 'Color'>;
    private quadVertexBuffer: WebGLBuffer;
    private stencilVertexPositionsBuffer: WebGLBuffer;
    private stencilVertexTileIndicesBuffer: WebGLBuffer;
    private stencilVertexArray: WebGLVertexArrayObject;
    private opaqueVertexBuffer: WebGLBuffer;
    private opaqueVertexArray: WebGLVertexArrayObject;
    private coverVertexBuffer: WebGLBuffer;
    private coverVertexArray: WebGLVertexArrayObject;

    private scene: Scene | null;
    private primitiveCount: number;
    private tileCount: number;
    private opaqueTileCount: number;

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
                      STENCIL_FRAMEBUFFER_SIZE.width,
                      STENCIL_FRAMEBUFFER_SIZE.height,
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

        const opaqueProgram = new Program(gl,
                                          OPAQUE_VERTEX_SHADER_SOURCE,
                                          OPAQUE_FRAGMENT_SHADER_SOURCE,
                                          ['FramebufferSize', 'TileSize'],
                                          ['TessCoord', 'TileOrigin', 'Color']);
        this.opaqueProgram = opaqueProgram;

        const stencilProgram = new Program(gl,
                                           STENCIL_VERTEX_SHADER_SOURCE,
                                           STENCIL_FRAGMENT_SHADER_SOURCE,
                                           ['FramebufferSize', 'TileSize', 'AreaLUT'],
                                           ['TessCoord', 'From', 'Ctrl', 'To', 'TileIndex']);
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
        gl.vertexAttribPointer(stencilProgram.attributes.From, 2, gl.FLOAT, false, 24, 0);
        gl.vertexAttribDivisor(stencilProgram.attributes.From, 1);
        gl.vertexAttribPointer(stencilProgram.attributes.Ctrl, 2, gl.FLOAT, false, 24, 8);
        gl.vertexAttribDivisor(stencilProgram.attributes.Ctrl, 1);
        gl.vertexAttribPointer(stencilProgram.attributes.To, 2, gl.FLOAT, false, 24, 16);
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
        gl.enableVertexAttribArray(stencilProgram.attributes.Ctrl);
        gl.enableVertexAttribArray(stencilProgram.attributes.To);
        gl.enableVertexAttribArray(stencilProgram.attributes.TileIndex);

        // Initialize opaque VBO.
        this.opaqueVertexBuffer = unwrapNull(gl.createBuffer());

        // Initialize opaque VAO.
        this.opaqueVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.opaqueVertexArray);
        gl.useProgram(this.opaqueProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.vertexAttribPointer(opaqueProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.opaqueVertexBuffer);
        gl.vertexAttribPointer(opaqueProgram.attributes.TileOrigin, 2, gl.SHORT, false, 10, 0);
        gl.vertexAttribDivisor(opaqueProgram.attributes.TileOrigin, 1);
        gl.vertexAttribPointer(opaqueProgram.attributes.Color, 4, gl.UNSIGNED_BYTE, true, 10, 6);
        gl.vertexAttribDivisor(opaqueProgram.attributes.Color, 1);
        gl.enableVertexAttribArray(opaqueProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(opaqueProgram.attributes.TileOrigin);
        gl.enableVertexAttribArray(opaqueProgram.attributes.Color);

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
        this.tileCount = 0;
        this.opaqueTileCount = 0;
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
        gl.viewport(0, 0, STENCIL_FRAMEBUFFER_SIZE.width, STENCIL_FRAMEBUFFER_SIZE.height);
        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        gl.bindVertexArray(this.stencilVertexArray);
        gl.useProgram(this.stencilProgram.program);
        gl.uniform2f(this.stencilProgram.uniforms.FramebufferSize,
                     STENCIL_FRAMEBUFFER_SIZE.width,
                     STENCIL_FRAMEBUFFER_SIZE.height);
        gl.uniform2f(this.stencilProgram.uniforms.TileSize, TILE_SIZE.width, TILE_SIZE.height);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.areaLUTTexture);
        gl.uniform1i(this.stencilProgram.uniforms.AreaLUT, 0);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, unwrapNull(this.primitiveCount));
        gl.disable(gl.BLEND);

        // Read back stencil and dump it.
        const totalStencilFramebufferSize = STENCIL_FRAMEBUFFER_SIZE.width *
            STENCIL_FRAMEBUFFER_SIZE.height * 4;
        const stencilData = new Float32Array(totalStencilFramebufferSize);
        gl.readPixels(0, 0,
                      STENCIL_FRAMEBUFFER_SIZE.width, STENCIL_FRAMEBUFFER_SIZE.height,
                      gl.RGBA,
                      gl.FLOAT,
                      stencilData);
        const stencilDumpData = new Uint8ClampedArray(totalStencilFramebufferSize);
        for (let i = 0; i < stencilData.length; i++)
            stencilDumpData[i] = stencilData[i] * 255.0;
        const stencilDumpCanvas = document.createElement('canvas');
        stencilDumpCanvas.width = STENCIL_FRAMEBUFFER_SIZE.width;
        stencilDumpCanvas.height = STENCIL_FRAMEBUFFER_SIZE.height;
        stencilDumpCanvas.style.width =
            (STENCIL_FRAMEBUFFER_SIZE.width / window.devicePixelRatio) + "px";
        stencilDumpCanvas.style.height =
            (STENCIL_FRAMEBUFFER_SIZE.height / window.devicePixelRatio) + "px";
        const stencilDumpCanvasContext = unwrapNull(stencilDumpCanvas.getContext('2d'));
        const stencilDumpImageData = new ImageData(stencilDumpData,
                                                   STENCIL_FRAMEBUFFER_SIZE.width,
                                                   STENCIL_FRAMEBUFFER_SIZE.height);
        stencilDumpCanvasContext.putImageData(stencilDumpImageData, 0, 0);
        document.body.appendChild(stencilDumpCanvas);
        //console.log(stencilData);

        // Draw opaque tiles.
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        const framebufferSize = {width: canvas.width, height: canvas.height};
        gl.viewport(0, 0, framebufferSize.width, framebufferSize.height);
        gl.clearColor(0.85, 0.85, 0.85, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        gl.bindVertexArray(this.opaqueVertexArray);
        gl.useProgram(this.opaqueProgram.program);
        gl.uniform2f(this.opaqueProgram.uniforms.FramebufferSize,
                     framebufferSize.width,
                     framebufferSize.height);
        gl.uniform2f(this.opaqueProgram.uniforms.TileSize, TILE_SIZE.width, TILE_SIZE.height);
        gl.disable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, this.opaqueTileCount);

        // Cover.
        gl.bindVertexArray(this.coverVertexArray);
        gl.useProgram(this.coverProgram.program);
        gl.uniform2f(this.coverProgram.uniforms.FramebufferSize,
                     framebufferSize.width,
                     framebufferSize.height);
        gl.uniform2f(this.coverProgram.uniforms.TileSize, TILE_SIZE.width, TILE_SIZE.height);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.stencilTexture);
        gl.uniform1i(this.coverProgram.uniforms.StencilTexture, 0);
        gl.uniform2f(this.coverProgram.uniforms.StencilTextureSize,
                     STENCIL_FRAMEBUFFER_SIZE.width,
                     STENCIL_FRAMEBUFFER_SIZE.height);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
        gl.enable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, this.tileCount);
        gl.disable(gl.BLEND);

        // End timer.
        if (timerQuery != null) {
            gl.endQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT);
            waitForQuery(gl, this.disjointTimerQueryExt, timerQuery);
        }
    }

    buildScene(): void {
        this.scene = new Scene(this.svg);
    }

    prepare(): void {
        const gl = this.gl, scene = unwrapNull(this.scene);

        // Construct opaque tile VBOs.
        this.opaqueTileCount = 0;
        const opaqueVertexData: number[] = [];
        const opaqueTiles: number[][] = [];
        for (let pathIndex = scene.pathTileStrips.length - 1; pathIndex >= 0; pathIndex--) {
            const pathTileStrips = scene.pathTileStrips[pathIndex];
            for (const tileStrip of pathTileStrips) {
                for (const tile of tileStrip.tiles) {
                    // TODO(pcwalton)
                    const color = scene.pathColors[pathIndex];
                    if (!tile.isFilled())
                        continue;

                    if (opaqueTiles[tile.tileLeft] == null)
                        opaqueTiles[tile.tileLeft] = [];
                    if (opaqueTiles[tile.tileLeft][tileStrip.tileTop] != null)
                        continue;
                    opaqueTiles[tile.tileLeft][tileStrip.tileTop] = pathIndex;

                    opaqueVertexData.push(Math.floor(tile.tileLeft),
                                          Math.floor(tileStrip.tileTop),
                                          0,
                                          color.r | (color.g << 8),
                                          color.b | (color.a << 8));
                    this.opaqueTileCount++;
                }
            }
        }

        // Construct stencil and cover VBOs.
        this.tileCount = 0;
        let primitiveCount = 0;
        const stencilVertexPositions: number[] = [], stencilVertexTileIndices: number[] = [];
        const coverVertexData: number[] = [];
        for (let pathIndex = 0; pathIndex < scene.pathTileStrips.length; pathIndex++) {
            const pathTileStrips = scene.pathTileStrips[pathIndex];
            for (const tileStrip of pathTileStrips) {
                for (const tile of tileStrip.tiles) {
                    const color = scene.pathColors[pathIndex];
                    if (tile.isFilled())
                        continue;

                    if (opaqueTiles[tile.tileLeft] != null &&
                            opaqueTiles[tile.tileLeft][tileStrip.tileTop] != null &&
                            pathIndex <= opaqueTiles[tile.tileLeft][tileStrip.tileTop]) {
                        continue;
                    }

                    for (const edge of tile.edges) {
                        let ctrl;
                        if (edge.ctrl == null)
                            ctrl = edge.from.lerp(edge.to, 0.5);
                        else
                            ctrl = edge.ctrl;
                        stencilVertexPositions.push(edge.from.x, edge.from.y,
                                                    ctrl.x, ctrl.y,
                                                    edge.to.x, edge.to.y);
                        stencilVertexTileIndices.push(this.tileCount);
                        primitiveCount++;
                    }

                    coverVertexData.push(Math.floor(tile.tileLeft),
                                         Math.floor(tileStrip.tileTop),
                                         this.tileCount,
                                         color.r | (color.g << 8),
                                         color.b | (color.a << 8));

                    this.tileCount++;
                }
            }
        }

        // Populate the stencil VBOs.
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(stencilVertexPositions), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.stencilVertexTileIndicesBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Uint16Array(stencilVertexTileIndices), gl.STATIC_DRAW);

        // Populate the opaque VBO.
        gl.bindBuffer(gl.ARRAY_BUFFER, this.opaqueVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Int16Array(opaqueVertexData), gl.DYNAMIC_DRAW);

        // Populate the cover VBO.
        gl.bindBuffer(gl.ARRAY_BUFFER, this.coverVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Int16Array(coverVertexData), gl.DYNAMIC_DRAW);
        //console.log(coverVertexData);

        this.primitiveCount = primitiveCount;
        console.log(primitiveCount + " primitives");
    }

    private onClick(event: MouseEvent): void {
        this.redraw();
    }
}

class Scene {
    pathTileStrips: TileStrip[][];
    pathColors: Color[];

    constructor(svg: XMLDocument) {
        const svgElement = unwrapNull(svg.documentElement).cloneNode(true);
        document.body.appendChild(svgElement);

        const pathElements = Array.from(document.getElementsByTagName('path'));
        const pathColors: any[] = [];

        this.pathTileStrips = [];

        //const tileDebugger = new TileDebugger(document);

        let fillCount = 0, strokeCount = 0;
        const paths: SVGPath[] = [];
        for (let pathElementIndex = 0;
             pathElementIndex < pathElements.length;
             pathElementIndex++) {
            const pathElement = pathElements[pathElementIndex];
            const pathString = unwrapNull(pathElement.getAttribute('d'));

            const style = window.getComputedStyle(pathElement);
            if (style.fill != null && style.fill !== 'none') {
                fillCount++;
                this.addPath(paths, pathColors, style.fill, pathString, null);
            }
            if (style.stroke != null && style.stroke !== 'none') {
                strokeCount++;
                const strokeWidth =
                    style.strokeWidth == null ? 1.0 : parseFloat(style.strokeWidth);
                this.addPath(paths, pathColors, style.stroke, pathString, strokeWidth);
            }
        }
        console.log("", fillCount, "fills,", strokeCount, "strokes");

        const startTime = window.performance.now();

        for (const path of paths) {
            const tiler = new Tiler(path);
            tiler.tile();
            //tileDebugger.addTiler(tiler, paint, "" + realPathIndex);
            //console.log("path", pathElementIndex, "tiles", tiler.getStrips());

            const pathTileStrips = tiler.getTileStrips();
            this.pathTileStrips.push(pathTileStrips);
        }

        const endTime = window.performance.now();
        console.log("elapsed time for tiling: " + (endTime - startTime) + "ms");

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

        const svgContainer = document.createElement('div');
        svgContainer.style.position = 'relative';
        svgContainer.style.width = "2000px";
        svgContainer.style.height = "2000px";
        //svgContainer.appendChild(tileDebugger.svg);
        document.body.appendChild(svgContainer);

        this.pathColors = pathColors;
    }

    private addPath(paths: SVGPath[],
                    pathColors: any[],
                    paint: string,
                    pathString: string,
                    strokeWidth: number | null):
                    void {
        const color = parseColor(paint).rgba;
        pathColors.push({
            r: color[0],
            g: color[1],
            b: color[2],
            a: Math.round(color[3] * 255.),
        });

        let path: SVGPath = SVGPath(pathString);
        path = path.matrix([
            GLOBAL_TRANSFORM.a, GLOBAL_TRANSFORM.b,
            GLOBAL_TRANSFORM.c, GLOBAL_TRANSFORM.d,
            GLOBAL_TRANSFORM.tx, GLOBAL_TRANSFORM.ty,
        ]);

        path = flattenPath(path);

        if (strokeWidth != null) {
            const outline = new Outline(path);
            outline.calculateNormals();
            outline.stroke(strokeWidth * GLOBAL_TRANSFORM.a);
            const strokedPathString = outline.toSVGPathString();

            /*
            const newSVG = staticCast(document.createElementNS(SVG_NS, 'svg'), SVGElement);
            newSVG.style.position = 'absolute';
            newSVG.style.left = "0";
            newSVG.style.top = "0";
            newSVG.style.width = "2000px";
            newSVG.style.height = "2000px";

            const newPath = document.createElementNS(SVG_NS, 'path');
            newPath.setAttribute('d', strokedPathString);
            newPath.setAttribute('fill',
                                 "rgba(" + color[0] + "," + color[1] + "," + color[2] + "," +
                                 color[3] + ")");
            newSVG.appendChild(newPath);
            document.body.appendChild(newSVG);
            */

            path = SVGPath(strokedPathString);
            path = makePathMonotonic(path);
        }

        paths.push(path);
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

function main(): void {
    window.fetch(SVG).then(svg => {
        svg.text().then(svgText => {
            //testIntervals();

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
