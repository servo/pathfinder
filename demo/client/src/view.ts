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
import {StemDarkeningMode, SubpixelAAType} from "./aa-strategy";
import {AAOptions} from './app-controller';
import PathfinderBufferTexture from './buffer-texture';
import {Camera} from "./camera";
import {EXTDisjointTimerQuery, QUAD_ELEMENTS, UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {Renderer} from './renderer';
import {PathfinderShaderProgram, SHADER_NAMES, ShaderMap} from './shader-loader';
import {ShaderProgramSource, UnlinkedShaderProgram} from './shader-loader';
import {expectNotNull, PathfinderError, UINT32_SIZE, unwrapNull} from './utils';

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

export const TIMINGS: {[name: string]: string} = {
    compositing: "Compositing",
    rendering: "Rendering",
};

export interface Timings {
    compositing: number;
    rendering: number;
}

declare class WebGLQuery {}

export abstract class PathfinderView {
    canvas: HTMLCanvasElement;

    protected abstract get camera(): Camera;

    private dirty: boolean;

    private pulseHandle: number;

    constructor() {
        this.dirty = false;
        this.canvas = unwrapNull(document.getElementById('pf-canvas')) as HTMLCanvasElement;
        window.addEventListener('resize', () => this.resizeToFit(false), false);
    }

    setDirty(): void {
        if (this.dirty)
            return;
        this.dirty = true;
        window.requestAnimationFrame(() => this.redraw());
    }

    zoomIn(): void {
        this.camera.zoomIn();
    }

    zoomOut(): void {
        this.camera.zoomOut();
    }

    zoomPulse(): void {
        if (this.pulseHandle) {
            window.cancelAnimationFrame(this.pulseHandle);
            this.pulseHandle = 0;
            return;
        }
        let c = 0;
        let d = 0.005;
        const self = this;
        function tick() {
            self.camera.zoom(1 + d);
            if (c++ % 200 === 0) {
                d *= -1;
            }
            self.pulseHandle = window.requestAnimationFrame(tick);
        }
        this.pulseHandle = window.requestAnimationFrame(tick);
    }

    protected resized(): void {
        this.setDirty();
    }

    protected redraw(): void {
        this.dirty = false;
    }

    protected resizeToFit(initialSize: boolean): void {
        if (!this.canvas.classList.contains('pf-pane')) {
            const windowWidth = window.innerWidth;
            const canvasTop = this.canvas.getBoundingClientRect().top;
            let height = window.scrollY + window.innerHeight - canvasTop;

            const nonoverlappingBottomBar =
                document.getElementById('pf-nonoverlapping-bottom-bar');
            if (nonoverlappingBottomBar != null) {
                const rect = nonoverlappingBottomBar.getBoundingClientRect();
                height -= window.innerHeight - rect.top;
            }

            const devicePixelRatio = window.devicePixelRatio;

            const canvasSize = new Float32Array([windowWidth, height]) as glmatrix.vec2;
            glmatrix.vec2.scale(canvasSize, canvasSize, devicePixelRatio);
            glmatrix.vec2.round(canvasSize, canvasSize);

            this.canvas.style.width = windowWidth + 'px';
            this.canvas.style.height = height + 'px';
            this.canvas.width = canvasSize[0];
            this.canvas.height = canvasSize[1];
        }

        this.resized();
    }
}

export abstract class DemoView extends PathfinderView implements RenderContext {
    readonly renderer: Renderer;

    gl: WebGLRenderingContext;

    shaderPrograms: ShaderMap<PathfinderShaderProgram>;
    gammaLUT: HTMLImageElement;

    instancedArraysExt: ANGLEInstancedArrays;
    textureHalfFloatExt: OESTextureHalfFloat;
    timerQueryExt: EXTDisjointTimerQuery;
    vertexArrayObjectExt: OESVertexArrayObject;

    quadPositionsBuffer: WebGLBuffer;
    quadTexCoordsBuffer: WebGLBuffer;
    quadElementsBuffer: WebGLBuffer;

    atlasRenderingTimerQuery: WebGLQuery;
    compositingTimerQuery: WebGLQuery;

    meshes: PathfinderMeshBuffers[];
    meshData: PathfinderMeshData[];

    get colorAlphaFormat(): GLenum {
        return this.gl.RGBA;
    }

    get renderContext(): RenderContext {
        return this;
    }

    protected colorBufferHalfFloatExt: any;

    private wantsScreenshot: boolean;

    /// NB: All subclasses are responsible for creating a renderer in their constructors.
    constructor(gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super();

        this.initContext();

        const shaderSource = this.compileShaders(commonShaderSource, shaderSources);
        this.shaderPrograms = this.linkShaders(shaderSource);

        this.gammaLUT = gammaLUT;

        this.wantsScreenshot = false;
    }

    attachMeshes(meshes: PathfinderMeshData[]): void {
        this.renderer.attachMeshes(meshes);
        this.setDirty();
    }

    initQuadVAO(attributes: any): void {
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadPositionsBuffer);
        this.gl.vertexAttribPointer(attributes.aPosition, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadTexCoordsBuffer);
        this.gl.vertexAttribPointer(attributes.aTexCoord, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.enableVertexAttribArray(attributes.aPosition);
        this.gl.enableVertexAttribArray(attributes.aTexCoord);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.quadElementsBuffer);
    }

    queueScreenshot(): void {
        this.wantsScreenshot = true;
        this.setDirty();
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           aaOptions: AAOptions):
                           void {
        this.renderer.setAntialiasingOptions(aaType, aaLevel, aaOptions);
    }

    protected resized(): void {
        super.resized();
        this.renderer.canvasResized();
    }

    protected initContext(): void {
        // Initialize the OpenGL context.
        this.gl = expectNotNull(this.canvas.getContext('webgl', { antialias: false, depth: true }),
                                "Failed to initialize WebGL! Check that your browser supports it.");
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

    protected redraw(): void {
        super.redraw();

        this.renderer.redraw();

        // Invoke the post-render hook.
        this.renderingFinished();

        // Take a screenshot if desired.
        if (this.wantsScreenshot) {
            this.wantsScreenshot = false;
            this.takeScreenshot();
        }
    }

    protected renderingFinished(): void {}

    private compileShaders(commonSource: string, shaderSources: ShaderMap<ShaderProgramSource>):
                           ShaderMap<UnlinkedShaderProgram> {
        const shaders: Partial<ShaderMap<Partial<UnlinkedShaderProgram>>> = {};

        for (const shaderKey of SHADER_NAMES) {
            for (const typeName of ['vertex', 'fragment'] as Array<'vertex' | 'fragment'>) {
                const type = {
                    fragment: this.gl.FRAGMENT_SHADER,
                    vertex: this.gl.VERTEX_SHADER,
                }[typeName];

                const source = shaderSources[shaderKey][typeName];
                const shader = this.gl.createShader(type);
                if (shader == null)
                    throw new PathfinderError("Failed to create shader!");

                this.gl.shaderSource(shader, commonSource + "\n#line 1\n" + source);
                this.gl.compileShader(shader);
                if (!this.gl.getShaderParameter(shader, this.gl.COMPILE_STATUS)) {
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
        const shaderProgramMap: Partial<ShaderMap<PathfinderShaderProgram>> = {};
        for (const shaderName of Object.keys(shaders) as Array<keyof ShaderMap<string>>) {
            shaderProgramMap[shaderName] = new PathfinderShaderProgram(this.gl,
                                                                       shaderName,
                                                                       shaders[shaderName]);
        }
        return shaderProgramMap as ShaderMap<PathfinderShaderProgram>;
    }

    private takeScreenshot(): void {
        const width = this.canvas.width, height = this.canvas.height;
        const scratchCanvas = document.createElement('canvas');
        scratchCanvas.width = width;
        scratchCanvas.height = height;
        const scratch2DContext = unwrapNull(scratchCanvas.getContext('2d'));
        scratch2DContext.drawImage(this.canvas, 0, 0, width, height);

        const scratchLink = document.createElement('a');
        scratchLink.download = 'pathfinder-screenshot.png';
        scratchLink.href = scratchCanvas.toDataURL();
        scratchLink.style.position = 'absolute';
        document.body.appendChild(scratchLink);
        scratchLink.click();
        document.body.removeChild(scratchLink);
    }
}

export interface RenderContext {
    /// The OpenGL context.
    readonly gl: WebGLRenderingContext;

    readonly instancedArraysExt: ANGLEInstancedArrays;
    readonly textureHalfFloatExt: OESTextureHalfFloat;
    readonly timerQueryExt: EXTDisjointTimerQuery;
    readonly vertexArrayObjectExt: OESVertexArrayObject;

    readonly colorAlphaFormat: GLenum;

    readonly shaderPrograms: ShaderMap<PathfinderShaderProgram>;
    readonly gammaLUT: HTMLImageElement;

    readonly quadPositionsBuffer: WebGLBuffer;
    readonly quadElementsBuffer: WebGLBuffer;

    readonly atlasRenderingTimerQuery: WebGLQuery;
    readonly compositingTimerQuery: WebGLQuery;

    initQuadVAO(attributes: any): void;
    setDirty(): void;
}
