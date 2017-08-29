// pathfinder/client/src/view.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {QUAD_ELEMENTS, UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {PathfinderShaderProgram, SHADER_NAMES, ShaderMap} from './shader-loader';
import {ShaderProgramSource, UnlinkedShaderProgram} from './shader-loader';
import {PathfinderError, UINT32_SIZE, expectNotNull, unwrapNull} from './utils';
import PathfinderBufferTexture from './buffer-texture';

const SCALE_FACTOR: number = 1.0 / 100.0;

const TIME_INTERVAL_DELAY: number = 32;

const B_LOOP_BLINN_DATA_SIZE: number = 4;
const B_LOOP_BLINN_DATA_TEX_COORD_OFFSET: number = 0;
const B_LOOP_BLINN_DATA_SIGN_OFFSET: number = 2;

const QUAD_POSITIONS: Float32Array = new Float32Array([
    0.0, 1.0,
    1.0, 1.0,
    0.0, 0.0,
    1.0, 0.0,
]);

const QUAD_TEX_COORDS: Float32Array = new Float32Array([
    0.0, 1.0,
    1.0, 1.0,
    0.0, 0.0,
    1.0, 0.0,
]);

export interface Timings {
    atlasRendering: number;
    compositing: number;
}

declare class WebGLQuery {}

export abstract class PathfinderView {
    constructor(canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        this.canvas = canvas;

        this.initContext();

        this.translation = glmatrix.vec2.create();

        const shaderSource = this.compileShaders(commonShaderSource, shaderSources);
        this.shaderPrograms = this.linkShaders(shaderSource);

        this.pathTransformBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathTransform');
        this.pathColorsBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathColors');

        this.antialiasingStrategy = new NoAAStrategy(0);
        this.antialiasingStrategy.init(this);

        window.addEventListener('resize', () => this.resizeToFit(false), false);
        this.resizeToFit(true);

        this.canvas.addEventListener('wheel', event => this.onWheel(event), false);
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName, aaLevel: number) {
        this.antialiasingStrategy = this.createAAStrategy(aaType, aaLevel);

        let canvas = this.canvas;
        this.antialiasingStrategy.init(this);
        if (this.meshData != null)
            this.antialiasingStrategy.attachMeshes(this);

        this.setDirty();
    }

    attachMeshes(meshes: PathfinderMeshData) {
        this.meshData = meshes;
        this.meshes = new PathfinderMeshBuffers(this.gl, meshes);
        this.antialiasingStrategy.attachMeshes(this);

        this.setDirty();
    }

    private resizeToFit(initialSize: boolean) {
        const width = window.innerWidth;
        const height = window.scrollY + window.innerHeight -
            this.canvas.getBoundingClientRect().top;
        const devicePixelRatio = window.devicePixelRatio;

        const canvasSize = new Float32Array([width, height]) as glmatrix.vec2;
        glmatrix.vec2.scale(canvasSize, canvasSize, devicePixelRatio);

        this.canvas.style.width = width + 'px';
        this.canvas.style.height = height + 'px';
        this.canvas.width = canvasSize[0];
        this.canvas.height = canvasSize[1];

        this.resized(initialSize);
    }

    protected abstract resized(initialSize: boolean): void;

    protected initContext() {
        // Initialize the OpenGL context.
        this.gl = expectNotNull(this.canvas.getContext('webgl', { antialias: false, depth: true }),
                                "Failed to initialize WebGL! Check that your browser supports it.");
        this.drawBuffersExt = this.gl.getExtension('WEBGL_draw_buffers');
        this.colorBufferHalfFloatExt = this.gl.getExtension('EXT_color_buffer_half_float');
        this.instancedArraysExt = this.gl.getExtension('ANGLE_instanced_arrays');
        this.textureHalfFloatExt = this.gl.getExtension('OES_texture_half_float');
        this.timerQueryExt = this.gl.getExtension('EXT_disjoint_timer_query');
        this.vertexArrayObjectExt = this.gl.getExtension('OES_vertex_array_object');
        this.gl.getExtension('EXT_frag_depth');
        this.gl.getExtension('OES_element_index_uint');
        this.gl.getExtension('OES_texture_float');
        this.gl.getExtension('WEBGL_depth_texture');

        // Upload quad buffers.
        this.quadPositionsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadPositionsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_POSITIONS, this.gl.STATIC_DRAW);
        this.quadTexCoordsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadTexCoordsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_TEX_COORDS, this.gl.STATIC_DRAW);
        this.quadElementsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.quadElementsBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, QUAD_ELEMENTS, this.gl.STATIC_DRAW);

        // Set up our timer queries for profiling.
        this.atlasRenderingTimerQuery = this.timerQueryExt.createQueryEXT();
        this.compositingTimerQuery = this.timerQueryExt.createQueryEXT();
    }

    private compileShaders(commonSource: string, shaderSources: ShaderMap<ShaderProgramSource>):
                           ShaderMap<UnlinkedShaderProgram> {
        let shaders: Partial<ShaderMap<Partial<UnlinkedShaderProgram>>> = {};

        for (const shaderKey of SHADER_NAMES) {
            for (const typeName of ['vertex', 'fragment'] as Array<'vertex' | 'fragment'>) {
                const type = {
                    vertex: this.gl.VERTEX_SHADER,
                    fragment: this.gl.FRAGMENT_SHADER,
                }[typeName];

                const source = shaderSources[shaderKey][typeName];
                const shader = this.gl.createShader(type);
                if (shader == null)
                    throw new PathfinderError("Failed to create shader!");

                this.gl.shaderSource(shader, commonSource + "\n#line 1\n" + source);
                this.gl.compileShader(shader);
                if (this.gl.getShaderParameter(shader, this.gl.COMPILE_STATUS) == 0) {
                    const infoLog = this.gl.getShaderInfoLog(shader);
                    throw new PathfinderError(`Failed to compile ${typeName} shader ` +
                                              `"${shaderKey}":\n${infoLog}`);
                }

                if (shaders[shaderKey] == null)
                    shaders[shaderKey] = {};
                shaders[shaderKey]![typeName] = shader;
            }
        }

        return shaders as ShaderMap<UnlinkedShaderProgram>;
    }

    private linkShaders(shaders: ShaderMap<UnlinkedShaderProgram>):
                        ShaderMap<PathfinderShaderProgram> {
        let shaderProgramMap: Partial<ShaderMap<PathfinderShaderProgram>> = {};
        for (const shaderName of Object.keys(shaders) as Array<keyof ShaderMap<string>>) {
            shaderProgramMap[shaderName] = new PathfinderShaderProgram(this.gl,
                                                                       shaderName,
                                                                       shaders[shaderName]);
        }
        return shaderProgramMap as ShaderMap<PathfinderShaderProgram>;
    }

    initQuadVAO(attributes: any) {
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadPositionsBuffer);
        this.gl.vertexAttribPointer(attributes.aPosition, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadTexCoordsBuffer);
        this.gl.vertexAttribPointer(attributes.aTexCoord, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.enableVertexAttribArray(attributes.aPosition);
        this.gl.enableVertexAttribArray(attributes.aTexCoord);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.quadElementsBuffer);
    }

    setFramebufferSizeUniform(uniforms: UniformMap) {
        const currentViewport = this.gl.getParameter(this.gl.VIEWPORT);
        this.gl.uniform2i(uniforms.uFramebufferSize, currentViewport[2], currentViewport[3]);
    }

    protected redraw() {
        if (this.meshes == null) {
            this.dirty = false;
            return;
        }

        // Start timing rendering.
        if (this.timerQueryPollInterval == null) {
            this.timerQueryExt.beginQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT,
                                             this.atlasRenderingTimerQuery);
        }

        // Prepare for direct rendering.
        this.antialiasingStrategy.prepare(this);

        // Perform direct rendering (Loop-Blinn).
        if (this.antialiasingStrategy.shouldRenderDirect)
            this.renderDirect();

        // Antialias.
        this.antialiasingStrategy.resolve(this);

        // End the timer, and start a new one.
        if (this.timerQueryPollInterval == null) {
            this.timerQueryExt.endQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT);
            this.timerQueryExt.beginQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT,
                                             this.compositingTimerQuery);
        }

        // Draw the glyphs with the resolved atlas to the default framebuffer.
        this.compositeIfNecessary();

        // Finish timing, clear dirty bit, and finish.
        this.finishTiming();
        this.dirty = false;
    }

    private setTransformUniform(uniforms: UniformMap) {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.mul(transform, this.antialiasingStrategy.transform, this.worldTransform);
        this.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);
    }

    private renderDirect() {
        // Set up implicit cover state.
        this.gl.depthFunc(this.gl.GREATER);
        this.gl.depthMask(true);
        this.gl.enable(this.gl.DEPTH_TEST);
        this.gl.disable(this.gl.BLEND);

        // Set up the implicit cover interior VAO.
        const directInteriorProgram = this.shaderPrograms.directInterior;
        this.gl.useProgram(directInteriorProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
        this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPosition,
                                    2,
                                    this.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPathIDs);
        this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPathID,
                                    1,
                                    this.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
        this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathID);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverInteriorIndices);

        // Draw direct interior parts.
        this.setTransformUniform(directInteriorProgram.uniforms);
        this.setFramebufferSizeUniform(directInteriorProgram.uniforms);
        this.pathColorsBufferTexture.bind(this.gl, directInteriorProgram.uniforms, 0);
        this.pathTransformBufferTexture.bind(this.gl, directInteriorProgram.uniforms, 1);
        let indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                    this.gl.BUFFER_SIZE) / UINT32_SIZE;
        this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);

        // Set up direct curve state.
        this.gl.depthMask(false);
        this.gl.enable(this.gl.BLEND);
        this.gl.blendEquation(this.gl.FUNC_ADD);
        this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);

        // Set up the direct curve VAO.
        const directCurveProgram = this.shaderPrograms.directCurve;
        this.gl.useProgram(directCurveProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aPosition,
                                    2,
                                    this.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPathIDs);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aPathID,
                                    1,
                                    this.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexLoopBlinnData);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aTexCoord,
                                    2,
                                    this.gl.UNSIGNED_BYTE,
                                    false,
                                    B_LOOP_BLINN_DATA_SIZE,
                                    B_LOOP_BLINN_DATA_TEX_COORD_OFFSET);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aSign,
                                    1,
                                    this.gl.BYTE,
                                    false,
                                    B_LOOP_BLINN_DATA_SIZE,
                                    B_LOOP_BLINN_DATA_SIGN_OFFSET);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aTexCoord);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPathID);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aSign);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverCurveIndices);

        // Draw direct curve parts.
        this.setTransformUniform(directCurveProgram.uniforms);
        this.setFramebufferSizeUniform(directCurveProgram.uniforms);
        this.pathColorsBufferTexture.bind(this.gl, directCurveProgram.uniforms, 0);
        this.pathTransformBufferTexture.bind(this.gl, directCurveProgram.uniforms, 1);
        indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                this.gl.BUFFER_SIZE) / UINT32_SIZE;
        this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);
    }

    private finishTiming() {
        if (this.timerQueryPollInterval != null)
            return;

        this.timerQueryExt.endQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT);

        this.timerQueryPollInterval = window.setInterval(() => {
            for (const queryName of ['atlasRenderingTimerQuery', 'compositingTimerQuery'] as
                    Array<'atlasRenderingTimerQuery' | 'compositingTimerQuery'>) {
                if (this.timerQueryExt.getQueryObjectEXT(this[queryName],
                                                         this.timerQueryExt
                                                               .QUERY_RESULT_AVAILABLE_EXT) == 0) {
                    return;
                }
            }

            const atlasRenderingTime =
                this.timerQueryExt.getQueryObjectEXT(this.atlasRenderingTimerQuery,
                                                     this.timerQueryExt.QUERY_RESULT_EXT);
            const compositingTime =
                this.timerQueryExt.getQueryObjectEXT(this.compositingTimerQuery,
                                                     this.timerQueryExt.QUERY_RESULT_EXT);
            this.updateTimings({
                atlasRendering: atlasRenderingTime / 1000000.0,
                compositing: compositingTime / 1000000.0,
            });

            window.clearInterval(this.timerQueryPollInterval!);
            this.timerQueryPollInterval = null;
        }, TIME_INTERVAL_DELAY);
    }

    protected setDirty() {
        if (this.dirty)
            return;
        this.dirty = true;
        window.requestAnimationFrame(() => this.redraw());
    }

    private onWheel(event: WheelEvent) {
        event.preventDefault();

        if (event.ctrlKey) {
            // Zoom event: see https://developer.mozilla.org/en-US/docs/Web/Events/wheel
            const mouseLocation = glmatrix.vec2.fromValues(event.clientX, event.clientY);
            const canvasLocation = this.canvas.getBoundingClientRect();
            mouseLocation[0] -= canvasLocation.left;
            mouseLocation[1] = canvasLocation.bottom - mouseLocation[1];
            glmatrix.vec2.scale(mouseLocation, mouseLocation, window.devicePixelRatio);

            const absoluteTranslation = glmatrix.vec2.create();
            glmatrix.vec2.sub(absoluteTranslation, this.translation, mouseLocation);
            glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, 1.0 / this.scale);

            this.scale *= 1.0 - event.deltaY * window.devicePixelRatio * SCALE_FACTOR;

            glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, this.scale);
            glmatrix.vec2.add(this.translation, absoluteTranslation, mouseLocation);
            return;
        }

        // Pan event.
        const delta = glmatrix.vec2.fromValues(-event.deltaX, event.deltaY);
        glmatrix.vec2.scale(delta, delta, window.devicePixelRatio);
        glmatrix.vec2.add(this.translation, this.translation, delta);

        this.panned();
    }

    setTransformSTAndTexScaleUniformsForDest(uniforms: UniformMap) {
        const usedSize = this.usedSizeFactor;
        this.gl.uniform4f(uniforms.uTransformST, 2.0 * usedSize[0], 2.0 * usedSize[1], -1.0, -1.0);
        this.gl.uniform2f(uniforms.uTexScale, usedSize[0], usedSize[1]);
    }

    setTransformAndTexScaleUniformsForDest(uniforms: UniformMap) {
        const usedSize = this.usedSizeFactor;

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 * usedSize[0], 2.0 * usedSize[1], 1.0]);
        this.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);

        this.gl.uniform2f(uniforms.uTexScale, usedSize[0], usedSize[1]);
    }

    protected abstract createAAStrategy(aaType: AntialiasingStrategyName, aaLevel: number):
                                        AntialiasingStrategy;

    protected abstract compositeIfNecessary(): void;

    protected abstract updateTimings(timings: Timings): void;

    protected abstract panned(): void;

    abstract get destFramebuffer(): WebGLFramebuffer | null;

    abstract get destAllocatedSize(): glmatrix.vec2;
    abstract get destUsedSize(): glmatrix.vec2;

    protected abstract get usedSizeFactor(): glmatrix.vec2;

    protected abstract get scale(): number;
    protected abstract set scale(newScale: number);

    protected abstract get worldTransform(): glmatrix.mat4;

    protected antialiasingStrategy: AntialiasingStrategy;

    protected translation: glmatrix.vec2;

    protected canvas: HTMLCanvasElement;

    gl: WebGLRenderingContext;

    shaderPrograms: ShaderMap<PathfinderShaderProgram>;

    protected colorBufferHalfFloatExt: any;
    drawBuffersExt: any;
    instancedArraysExt: any;
    textureHalfFloatExt: any;
    protected timerQueryExt: any;
    vertexArrayObjectExt: any;

    quadPositionsBuffer: WebGLBuffer;
    quadTexCoordsBuffer: WebGLBuffer;
    quadElementsBuffer: WebGLBuffer;

    meshes: PathfinderMeshBuffers;
    meshData: PathfinderMeshData;

    pathTransformBufferTexture: PathfinderBufferTexture;
    protected pathColorsBufferTexture: PathfinderBufferTexture;

    private atlasRenderingTimerQuery: WebGLQuery;
    private compositingTimerQuery: WebGLQuery;
    private timerQueryPollInterval: number | null;

    protected dirty: boolean;
}

export abstract class MonochromePathfinderView extends PathfinderView {
    abstract get bgColor(): glmatrix.vec4;
    abstract get fgColor(): glmatrix.vec4;
}
