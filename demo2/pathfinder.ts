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
import AREA_LUT from "../resources/textures/area-lut.png";
import {Matrix2D, Size2D, Rect, Point2D} from "./geometry";
import {SVGPath, TILE_SIZE} from "./tiling";
import {staticCast, unwrapNull} from "./util";

const SVGPath: (path: string) => SVGPath = require('svgpath');

const STENCIL_FRAMEBUFFER_SIZE: Size2D = {
    width: TILE_SIZE.width * 256,
    height: TILE_SIZE.height * 256,
};

const QUAD_VERTEX_POSITIONS: Uint8Array = new Uint8Array([
    0, 0,
    1, 0,
    1, 1,
    0, 1,
]);

const FILL_INSTANCE_SIZE: number = 8;
const SOLID_TILE_INSTANCE_SIZE: number = 6;
const MASK_TILE_INSTANCE_SIZE: number = 8;

interface Color {
    r: number;
    g: number;
    b: number;
    a: number;
}

type Edge = 'left' | 'top' | 'right' | 'bottom';

type FillProgram =
    Program<'FramebufferSize' | 'TileSize' | 'AreaLUT',
            'TessCoord' | 'FromPx' | 'ToPx' | 'FromSubpx' | 'ToSubpx' | 'TileIndex'>;
type SolidTileProgram =
    Program<'FramebufferSize' |
            'TileSize' |
            'FillColorsTexture' | 'FillColorsTextureSize' |
            'ViewBoxOrigin',
            'TessCoord' | 'TileOrigin' | 'Object'>;
type MaskTileProgram =
    Program<'FramebufferSize' |
            'TileSize' |
            'StencilTexture' | 'StencilTextureSize' |
            'FillColorsTexture' | 'FillColorsTextureSize' |
            'ViewBoxOrigin',
            'TessCoord' | 'TileOrigin' | 'Backdrop' | 'Object'>;

class App {
    private canvas: HTMLCanvasElement;
    private openButton: HTMLInputElement;
    private areaLUT: HTMLImageElement;

    private gl: WebGL2RenderingContext;
    private disjointTimerQueryExt: any;
    private areaLUTTexture: WebGLTexture;
    private fillColorsTexture: WebGLTexture;
    private stencilTexture: WebGLTexture;
    private stencilFramebuffer: WebGLFramebuffer;
    private fillProgram: FillProgram;
    private solidTileProgram: SolidTileProgram;
    private maskTileProgram: MaskTileProgram;
    private quadVertexBuffer: WebGLBuffer;
    private solidTileVertexBuffer: WebGLBuffer;
    private solidVertexArray: WebGLVertexArrayObject;
    private batchBuffers: BatchBuffers[];

    private viewBox: Rect;

    private solidTileCount: number;
    private objectCount: number;

    constructor(areaLUT: HTMLImageElement) {
        const canvas = staticCast(document.getElementById('canvas'), HTMLCanvasElement);
        const openButton = staticCast(document.getElementById('open'), HTMLInputElement);
        this.canvas = canvas;
        this.openButton = openButton;
        this.areaLUT = areaLUT;

        this.openButton.addEventListener('change', event => this.loadFile(), false);

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

        this.fillColorsTexture = unwrapNull(gl.createTexture());

        /*
        const benchData = new Uint8Array(1600 * 1600 * 4);
        for (let i = 0; i < benchData.length; i++)
            benchData[i] = (Math.random() * 256) | 0;
        const benchTexture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, benchTexture);
        const startTime = performance.now();
        for (let i = 0; i < 100; i++)
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1600, 1600, 0, gl.RGBA, gl.UNSIGNED_BYTE, benchData);
        const elapsedTime = (performance.now() - startTime) / 100;
        console.log("texture upload: ", elapsedTime, "ms");
        */

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

        const maskTileProgram = new Program(gl,
                                         COVER_VERTEX_SHADER_SOURCE,
                                         COVER_FRAGMENT_SHADER_SOURCE,
                                         [
                                             'FramebufferSize',
                                             'TileSize',
                                             'StencilTexture',
                                             'StencilTextureSize',
                                             'FillColorsTexture',
                                             'FillColorsTextureSize',
                                             'ViewBoxOrigin',
                                         ],
                                         [
                                             'TessCoord',
                                             'TileOrigin',
                                             'TileIndex',
                                             'Backdrop',
                                             'Object',
                                         ]);
        this.maskTileProgram = maskTileProgram;

        const solidTileProgram = new Program(gl,
                                             OPAQUE_VERTEX_SHADER_SOURCE,
                                             OPAQUE_FRAGMENT_SHADER_SOURCE,
                                             [
                                                 'FramebufferSize',
                                                 'TileSize',
                                                 'FillColorsTexture',
                                                 'FillColorsTextureSize',
                                                 'ViewBoxOrigin',
                                             ],
                                             ['TessCoord', 'TileOrigin', 'Object']);
        this.solidTileProgram = solidTileProgram;

        const fillProgram = new Program(gl,
                                        STENCIL_VERTEX_SHADER_SOURCE,
                                        STENCIL_FRAGMENT_SHADER_SOURCE,
                                        ['FramebufferSize', 'TileSize', 'AreaLUT'],
                                        [
                                            'TessCoord',
                                            'FromPx', 'ToPx',
                                            'FromSubpx', 'ToSubpx',
                                            'TileIndex'
                                        ]);
        this.fillProgram = fillProgram;

        // Initialize quad VBO.
        this.quadVertexBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, QUAD_VERTEX_POSITIONS, gl.STATIC_DRAW);

        // Initialize tile VBOs and IBOs.
        this.solidTileVertexBuffer = unwrapNull(gl.createBuffer());

        // Initialize solid tile VAO.
        this.solidVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.solidVertexArray);
        gl.useProgram(this.solidTileProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.quadVertexBuffer);
        gl.vertexAttribPointer(solidTileProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.solidTileVertexBuffer);
        gl.vertexAttribPointer(solidTileProgram.attributes.TileOrigin,
                               2,
                               gl.SHORT,
                               false,
                               SOLID_TILE_INSTANCE_SIZE,
                               0);
        gl.vertexAttribDivisor(solidTileProgram.attributes.TileOrigin, 1);
        gl.vertexAttribIPointer(solidTileProgram.attributes.Object,
                                1,
                                gl.UNSIGNED_SHORT,
                                SOLID_TILE_INSTANCE_SIZE,
                                4);
        gl.vertexAttribDivisor(solidTileProgram.attributes.Object, 1);
        gl.enableVertexAttribArray(solidTileProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(solidTileProgram.attributes.TileOrigin);
        gl.enableVertexAttribArray(solidTileProgram.attributes.Object);

        this.batchBuffers = [];

        this.viewBox = new Rect(new Point2D(0.0, 0.0), new Size2D(0.0, 0.0));

        // Set up event handlers.
        this.canvas.addEventListener('click', event => this.onClick(event), false);

        this.solidTileCount = 0;
        this.objectCount = 0;
    }

    redraw(): void {
        const gl = this.gl, canvas = this.canvas;

        //console.log("viewBox", this.viewBox);

        // Initialize timers.
        let fillTimerQuery = null, solidTimerQuery = null, maskTimerQuery = null;
        if (this.disjointTimerQueryExt != null) {
            fillTimerQuery = unwrapNull(gl.createQuery());
            solidTimerQuery = unwrapNull(gl.createQuery());
            maskTimerQuery = unwrapNull(gl.createQuery());
        }

        // Clear.
        if (solidTimerQuery != null)
            gl.beginQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT, solidTimerQuery);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        const framebufferSize = {width: canvas.width, height: canvas.height};
        gl.viewport(0, 0, framebufferSize.width, framebufferSize.height);
        gl.clearColor(0.85, 0.85, 0.85, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        // Draw solid tiles.
        gl.bindVertexArray(this.solidVertexArray);
        gl.useProgram(this.solidTileProgram.program);
        gl.uniform2f(this.solidTileProgram.uniforms.FramebufferSize,
                     framebufferSize.width,
                     framebufferSize.height);
        gl.uniform2f(this.solidTileProgram.uniforms.TileSize, TILE_SIZE.width, TILE_SIZE.height);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.fillColorsTexture);
        gl.uniform1i(this.solidTileProgram.uniforms.FillColorsTexture, 0);
        // FIXME(pcwalton): Maybe this should be an ivec2 or uvec2?
        gl.uniform2f(this.solidTileProgram.uniforms.FillColorsTextureSize,
                     this.objectCount,
                     1.0);
        gl.uniform2f(this.solidTileProgram.uniforms.ViewBoxOrigin,
                     this.viewBox.origin.x,
                     this.viewBox.origin.y);
        gl.disable(gl.BLEND);
        gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, this.solidTileCount);
        if (solidTimerQuery != null)
            gl.endQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT);

        // Draw batches.
        if (fillTimerQuery != null)
            gl.beginQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT, fillTimerQuery);
        for (const batch of this.batchBuffers) {
            // Fill.
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.stencilFramebuffer);
            gl.viewport(0, 0, STENCIL_FRAMEBUFFER_SIZE.width, STENCIL_FRAMEBUFFER_SIZE.height);
            gl.clearColor(0.0, 0.0, 0.0, 0.0);
            gl.clear(gl.COLOR_BUFFER_BIT);

            gl.bindVertexArray(batch.fillVertexArray);
            gl.useProgram(this.fillProgram.program);
            gl.uniform2f(this.fillProgram.uniforms.FramebufferSize,
                        STENCIL_FRAMEBUFFER_SIZE.width,
                        STENCIL_FRAMEBUFFER_SIZE.height);
            gl.uniform2f(this.fillProgram.uniforms.TileSize, TILE_SIZE.width, TILE_SIZE.height);
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, this.areaLUTTexture);
            gl.uniform1i(this.fillProgram.uniforms.AreaLUT, 0);
            gl.blendEquation(gl.FUNC_ADD);
            gl.blendFunc(gl.ONE, gl.ONE);
            gl.enable(gl.BLEND);
            gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, unwrapNull(batch.fillPrimitiveCount));
            gl.disable(gl.BLEND);

            // Read back stencil and dump it.
            //this.dumpStencil();

            // Draw masked tiles.
            gl.bindFramebuffer(gl.FRAMEBUFFER, null);
            gl.viewport(0, 0, framebufferSize.width, framebufferSize.height);
            gl.bindVertexArray(batch.maskVertexArray);
            gl.useProgram(this.maskTileProgram.program);
            gl.uniform2f(this.maskTileProgram.uniforms.FramebufferSize,
                        framebufferSize.width,
                        framebufferSize.height);
            gl.uniform2f(this.maskTileProgram.uniforms.TileSize,
                         TILE_SIZE.width,
                         TILE_SIZE.height);
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, this.stencilTexture);
            gl.uniform1i(this.maskTileProgram.uniforms.StencilTexture, 0);
            gl.uniform2f(this.maskTileProgram.uniforms.StencilTextureSize,
                        STENCIL_FRAMEBUFFER_SIZE.width,
                        STENCIL_FRAMEBUFFER_SIZE.height);
            gl.activeTexture(gl.TEXTURE1);
            gl.bindTexture(gl.TEXTURE_2D, this.fillColorsTexture);
            gl.uniform1i(this.maskTileProgram.uniforms.FillColorsTexture, 1);
            // FIXME(pcwalton): Maybe this should be an ivec2 or uvec2?
            gl.uniform2f(this.maskTileProgram.uniforms.FillColorsTextureSize,
                        this.objectCount,
                        1.0);
            gl.uniform2f(this.maskTileProgram.uniforms.ViewBoxOrigin,
                        this.viewBox.origin.x,
                        this.viewBox.origin.y);
            gl.blendEquation(gl.FUNC_ADD);
            gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
            gl.enable(gl.BLEND);
            gl.drawArraysInstanced(gl.TRIANGLE_FAN, 0, 4, batch.maskTileCount);
            gl.disable(gl.BLEND);
        }
        if (fillTimerQuery != null)
            gl.endQuery(this.disjointTimerQueryExt.TIME_ELAPSED_EXT);

        // End timer.
        if (fillTimerQuery != null && solidTimerQuery != null) {
            processQueries(gl, this.disjointTimerQueryExt, {
                fill: fillTimerQuery,
                solid: solidTimerQuery,
            });
        }
    }

    private dumpStencil(): void {
        const gl = this.gl;

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
    }

    private loadFile(): void {
        console.log("loadFile");
        // TODO(pcwalton)
        const file = unwrapNull(unwrapNull(this.openButton.files)[0]);
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            const gl = this.gl;
            const arrayBuffer = staticCast(reader.result, ArrayBuffer);
            const root = new RIFFChunk(new DataView(arrayBuffer));
            for (const subchunk of root.subchunks(4)) {
                const self = this;

                const id = subchunk.stringID();
                if (id === 'head') {
                    const headerData = subchunk.contents();
                    const version = headerData.getUint32(0, true);
                    if (version !== 0)
                        throw new Error("Unknown version!");
                    // Ignore the batch count and fetch the view box.
                    this.viewBox = new Rect(new Point2D(headerData.getFloat32(8, true),
                                                        headerData.getFloat32(12, true)),
                                            new Size2D(headerData.getFloat32(16, true),
                                                       headerData.getFloat32(20, true)));
                    continue;
                } else if (id === 'soli') {
                    self.solidTileCount = uploadArrayBuffer(subchunk,
                                                            this.solidTileVertexBuffer,
                                                            SOLID_TILE_INSTANCE_SIZE);
                } else if (id === 'shad') {
                    this.objectCount = subchunk.length() / 4;
                    gl.activeTexture(gl.TEXTURE0);
                    gl.bindTexture(gl.TEXTURE_2D, this.fillColorsTexture);
                    const textureDataView = subchunk.contents();
                    const textureData = new Uint8Array(textureDataView.buffer,
                                                       textureDataView.byteOffset,
                                                       textureDataView.byteLength);
                    gl.texImage2D(gl.TEXTURE_2D,
                                  0,
                                  gl.RGBA,
                                  this.objectCount,
                                  1,
                                  0,
                                  gl.RGBA,
                                  gl.UNSIGNED_BYTE,
                                  textureData);
                    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
                    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
                    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
                    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
                } else if (id === 'batc') {
                    const batch = new BatchBuffers(this.gl,
                                                   this.fillProgram,
                                                   this.maskTileProgram,
                                                   this.quadVertexBuffer);
                    for (const subsubchunk of subchunk.subchunks()) {
                        const id = subsubchunk.stringID();
                        console.log("id=", id);
                        if (id === 'fill') {
                            batch.fillPrimitiveCount = uploadArrayBuffer(subsubchunk,
                                                                         batch.fillVertexBuffer,
                                                                         FILL_INSTANCE_SIZE);
                        } else if (id === 'mask') {
                            batch.maskTileCount = uploadArrayBuffer(subsubchunk,
                                                                    batch.maskTileVertexBuffer,
                                                                    MASK_TILE_INSTANCE_SIZE);
                        }
                    }

                    this.batchBuffers.push(batch);
                }

                function uploadArrayBuffer(chunk: RIFFChunk,
                                           buffer: WebGLBuffer,
                                           instanceSize: number):
                                           number {
                    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
                    gl.bufferData(gl.ARRAY_BUFFER, chunk.contents(), gl.DYNAMIC_DRAW);
                    return chunk.length() / instanceSize;
                }
            }

            this.redraw();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    private onClick(event: MouseEvent): void {
        this.redraw();
    }
}

class BatchBuffers {
    fillVertexBuffer: WebGLBuffer;
    fillVertexArray: WebGLVertexArrayObject;
    maskTileVertexBuffer: WebGLBuffer;
    maskVertexArray: WebGLVertexArrayObject;
    fillPrimitiveCount: number;
    maskTileCount: number;

    constructor(gl: WebGL2RenderingContext,
                fillProgram: FillProgram,
                maskTileProgram: MaskTileProgram,
                quadVertexBuffer: WebGLBuffer) {
        // Initialize fill VBOs.
        this.fillVertexBuffer = unwrapNull(gl.createBuffer());

        // Initialize fill VAO.
        this.fillVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.fillVertexArray);
        gl.useProgram(fillProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, quadVertexBuffer);
        gl.vertexAttribPointer(fillProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.fillVertexBuffer);
        gl.vertexAttribIPointer(fillProgram.attributes.FromPx,
                                1,
                                gl.UNSIGNED_BYTE,
                                FILL_INSTANCE_SIZE,
                                0);
        gl.vertexAttribDivisor(fillProgram.attributes.FromPx, 1);
        gl.vertexAttribIPointer(fillProgram.attributes.ToPx,
                                1,
                                gl.UNSIGNED_BYTE,
                                FILL_INSTANCE_SIZE,
                                1);
        gl.vertexAttribDivisor(fillProgram.attributes.ToPx, 1);
        gl.vertexAttribPointer(fillProgram.attributes.FromSubpx,
                               2,
                               gl.UNSIGNED_BYTE,
                               true,
                               FILL_INSTANCE_SIZE,
                               2);
        gl.vertexAttribDivisor(fillProgram.attributes.FromSubpx, 1);
        gl.vertexAttribPointer(fillProgram.attributes.ToSubpx,
                               2,
                               gl.UNSIGNED_BYTE,
                               true,
                               FILL_INSTANCE_SIZE,
                               4);
        gl.vertexAttribDivisor(fillProgram.attributes.ToSubpx, 1);
        gl.vertexAttribIPointer(fillProgram.attributes.TileIndex,
                                1,
                                gl.UNSIGNED_SHORT,
                                FILL_INSTANCE_SIZE,
                                6);
        gl.vertexAttribDivisor(fillProgram.attributes.TileIndex, 1);
        gl.enableVertexAttribArray(fillProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(fillProgram.attributes.FromPx);
        gl.enableVertexAttribArray(fillProgram.attributes.ToPx);
        gl.enableVertexAttribArray(fillProgram.attributes.FromSubpx);
        gl.enableVertexAttribArray(fillProgram.attributes.ToSubpx);
        gl.enableVertexAttribArray(fillProgram.attributes.TileIndex);

        // Initialize tile VBOs.
        this.maskTileVertexBuffer = unwrapNull(gl.createBuffer());

        // Initialize mask tile VAO.
        this.maskVertexArray = unwrapNull(gl.createVertexArray());
        gl.bindVertexArray(this.maskVertexArray);
        gl.useProgram(maskTileProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, quadVertexBuffer);
        gl.vertexAttribPointer(maskTileProgram.attributes.TessCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               0,
                               0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.maskTileVertexBuffer);
        gl.vertexAttribPointer(maskTileProgram.attributes.TileOrigin,
                               2,
                               gl.SHORT,
                               false,
                               MASK_TILE_INSTANCE_SIZE,
                               0);
        gl.vertexAttribDivisor(maskTileProgram.attributes.TileOrigin, 1);
        gl.vertexAttribIPointer(maskTileProgram.attributes.Backdrop,
                                1,
                                gl.SHORT,
                                MASK_TILE_INSTANCE_SIZE,
                                4);
        gl.vertexAttribDivisor(maskTileProgram.attributes.Backdrop, 1);
        gl.vertexAttribIPointer(maskTileProgram.attributes.Object,
                                1,
                                gl.UNSIGNED_SHORT,
                                MASK_TILE_INSTANCE_SIZE,
                                6);
        gl.vertexAttribDivisor(maskTileProgram.attributes.Object, 1);
        gl.enableVertexAttribArray(maskTileProgram.attributes.TessCoord);
        gl.enableVertexAttribArray(maskTileProgram.attributes.TileOrigin);
        gl.enableVertexAttribArray(maskTileProgram.attributes.Backdrop);
        gl.enableVertexAttribArray(maskTileProgram.attributes.Object);

        this.fillPrimitiveCount = 0;
        this.maskTileCount = 0;
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

class RIFFChunk {
    private data: DataView;

    constructor(data: DataView) {
        this.data = data;
    }

    stringID(): string {
        return String.fromCharCode(this.data.getUint8(0),
                                   this.data.getUint8(1),
                                   this.data.getUint8(2),
                                   this.data.getUint8(3));
    }

    length(): number {
        return this.data.getUint32(4, true);
    }

    contents(): DataView {
        return new DataView(this.data.buffer, this.data.byteOffset + 8, this.length());
    }

    subchunks(initialOffset?: number | undefined): RIFFChunk[] {
        const subchunks = [];
        const contents = this.contents(), length = this.length();
        let offset = initialOffset == null ? 0 : initialOffset;
        while (offset < length) {
            const subchunk = new RIFFChunk(new DataView(contents.buffer,
                                                        contents.byteOffset + offset,
                                                        length - offset));
            subchunks.push(subchunk);
            offset += subchunk.length() + 8;
        }
        return subchunks;
    }
}

interface Queries {
    fill: WebGLQuery;
    solid: WebGLQuery;
};

function getQueryResult(gl: WebGL2RenderingContext, disjointTimerQueryExt: any, query: WebGLQuery):
                        Promise<number> {
    function go(resolve: (n: number) => void): void {
        const queryResultAvailable = disjointTimerQueryExt.QUERY_RESULT_AVAILABLE_EXT;
        const queryResult = disjointTimerQueryExt.QUERY_RESULT_EXT;
        if (!disjointTimerQueryExt.getQueryObjectEXT(query, queryResultAvailable)) {
            setTimeout(() => go(resolve), 10);
            return;
        }
        resolve(disjointTimerQueryExt.getQueryObjectEXT(query, queryResult) / 1000000.0);
    }

    return new Promise((resolve, reject) => go(resolve));
}

function processQueries(gl: WebGL2RenderingContext, disjointTimerQueryExt: any, queries: Queries):
                        void {
    Promise.all([
        getQueryResult(gl, disjointTimerQueryExt, queries.fill),
        getQueryResult(gl, disjointTimerQueryExt, queries.solid),
    ]).then(results => {
        const [fillResult, solidResult] = results;
        console.log(fillResult, "ms fill/mask,", solidResult, "ms solid");
    });
}

function loadAreaLUT(): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {;
        const image = new Image;
        image.src = AREA_LUT;
        image.addEventListener('load', event => resolve(image), false);
    });
}

function main(): void {
    loadAreaLUT().then(image => new App(image));
}

document.addEventListener('DOMContentLoaded', () => main(), false);
