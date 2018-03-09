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
import {PathfinderPackedMeshBuffers, PathfinderPackedMeshes} from './meshes';
import {Renderer} from './renderer';
import {PathfinderShaderProgram, SHADER_NAMES, ShaderMap} from './shader-loader';
import {ShaderProgramSource, UnlinkedShaderProgram} from './shader-loader';
import {expectNotNull, PathfinderError, UINT32_SIZE, unwrapNull} from './utils';
import {NEAR_CLIP_PLANE, FAR_CLIP_PLANE} from './3d-demo';

const QUAD_POSITIONS: Float32Array = new Float32Array([
    0.0, 0.0,
    1.0, 0.0,
    0.0, 1.0,
    1.0, 1.0,
]);

const QUAD_TEX_COORDS: Float32Array = new Float32Array([
    0.0, 0.0,
    1.0, 0.0,
    0.0, 1.0,
    1.0, 1.0,
]);

export const TIMINGS: {[name: string]: string} = {
    compositing: "Compositing",
    rendering: "Rendering",
};

export interface Timings {
    compositing: number;
    rendering: number;
}

export type ColorAlphaFormat = 'RGBA8' | 'RGB5_A1';

declare class WebGLQuery {}

export abstract class PathfinderView {
    canvas: HTMLCanvasElement;

    suppressAutomaticRedraw: boolean;

    protected abstract get camera(): Camera;

    private dirty: boolean;

    private pulseHandle: number;

    constructor() {
        this.dirty = false;
        this.pulseHandle = 0;
        this.suppressAutomaticRedraw = false;
        this.canvas = unwrapNull(document.getElementById('pf-canvas')) as HTMLCanvasElement;
        window.addEventListener('resize', () => this.resizeToFit(false), false);
    }

    setDirty(): void {
        if (this.dirty || this.suppressAutomaticRedraw)
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

    rotate(newAngle: number): void {
        this.camera.rotate(newAngle);
    }

    redraw(): void {
        this.dirty = false;
    }

    protected resized(): void {
        this.setDirty();
    }

    protected resizeToFit(initialSize: boolean): void {
        if (!this.canvas.classList.contains('pf-no-autoresize')) {
            const windowWidth = window.innerWidth;
            const canvasTop = this.canvas.getBoundingClientRect().top;
            const height = window.scrollY + window.innerHeight - canvasTop;

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
    readonly renderer!: Renderer;

    gl!: WebGLRenderingContext;

    shaderPrograms: ShaderMap<PathfinderShaderProgram>;
    gammaLUT: HTMLImageElement;

    instancedArraysExt!: ANGLE_instanced_arrays;
    textureHalfFloatExt!: OESTextureHalfFloat;
    timerQueryExt!: EXTDisjointTimerQuery;
    vertexArrayObjectExt!: OESVertexArrayObject;

    quadPositionsBuffer!: WebGLBuffer;
    quadTexCoordsBuffer!: WebGLBuffer;
    quadElementsBuffer!: WebGLBuffer;


    meshes: PathfinderPackedMeshBuffers[];
    meshData: PathfinderPackedMeshes[];

    get colorAlphaFormat(): ColorAlphaFormat {
        // On macOS, RGBA framebuffers seem to cause driver stalls when switching between rendering
        // and texturing. Work around this by using RGB5A1 instead.
        return navigator.platform === 'MacIntel' ? 'RGB5_A1' : 'RGBA8';
    }

    get renderContext(): RenderContext {
        return this;
    }

    protected colorBufferHalfFloatExt: any;

    private wantsScreenshot: boolean;
    private vrDisplay: VRDisplay | null;
    private vrFrameData: VRFrameData | null;
    private inVrRAF: boolean;

    /// NB: All subclasses are responsible for creating a renderer in their constructors.
    constructor(gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super();

        this.meshes = [];
        this.meshData = [];

        this.initContext();

        const shaderSource = this.compileShaders(commonShaderSource, shaderSources);
        this.shaderPrograms = this.linkShaders(shaderSource);

        this.gammaLUT = gammaLUT;

        this.wantsScreenshot = false;
        this.vrDisplay = null;
        if ("VRFrameData" in window) {
           this.vrFrameData = new VRFrameData;
        } else {
            this.vrFrameData = null;
        }
        
        this.inVrRAF = false;
    }

    attachMeshes(meshes: PathfinderPackedMeshes[]): void {
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

    snapshot(rect: glmatrix.vec4): Uint8Array {
        const gl = this.renderContext.gl;
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);

        const canvasHeight = this.canvas.height;
        const width = rect[2] - rect[0], height = rect[3] - rect[1];
        const originX = Math.max(rect[0], 0);
        const originY = Math.max(canvasHeight - height, 0);
        const flippedBuffer = new Uint8Array(width * height * 4);
        gl.readPixels(originX, originY, width, height, gl.RGBA, gl.UNSIGNED_BYTE, flippedBuffer);

        const buffer = new Uint8Array(width * height * 4);
        for (let y = 0; y < height; y++) {
            const destRowStart = y * width * 4;
            const srcRowStart = (height - y - 1) * width * 4;
            buffer.set(flippedBuffer.slice(srcRowStart, srcRowStart + width * 4),
                       destRowStart);
        }

        return buffer;
    }

    enterVR(): void {
        if (this.vrDisplay != null) {
            this.vrDisplay.requestPresent([{ source: this.canvas }]);
            const that = this;
            function vrCallback(): void {
                if (that.vrDisplay == null) {
                    return;
                }
                that.vrDisplay.requestAnimationFrame(vrCallback);

                that.renderer.enterVR();
                that.inVrRAF = true;
                that.redraw();
                that.inVrRAF = false;
            }
            this.vrDisplay.requestAnimationFrame(vrCallback);
        }
        if (navigator.getVRDisplays) {
            navigator.getVRDisplays().then((displays) => {
              if (displays.length > 0) {
                this.vrDisplay = displays[displays.length - 1];

                // It's heighly reccommended that you set the near and far planes to
                // something appropriate for your scene so the projection matricies
                // WebVR produces have a well scaled depth buffer.
                this.vrDisplay.depthNear = NEAR_CLIP_PLANE;
                this.vrDisplay.depthFar = FAR_CLIP_PLANE;
              } else {
                alert("Your device has no VR displays")
              }
            }, function () {
              alert("Your browser does not support WebVR. See <a href='http://webvr.info'>webvr.info</a> for assistance.");
            });
        } else {
            alert("Your browser does not support WebVR. See <a href='http://webvr.info'>webvr.info</a> for assistance.");
        }
    }

    redraw(): void {
        super.redraw();

        if (!this.renderer.meshesAttached)
            return;

        if (this.vrDisplay == null || this.vrFrameData == null) {
            this.renderer.redraw();
        } else {
            if (!this.inVrRAF) {
                console.log("redraw() called outside of vr RAF, will get drawn later");
                return;
            }
            this.vrDisplay.getFrameData(this.vrFrameData);
            this.renderer.redrawVR(this.vrFrameData);
            this.vrDisplay.submitFrame()
        }

        // Invoke the post-render hook.
        this.renderingFinished();

        // Take a screenshot if desired.
        if (this.wantsScreenshot) {
            this.wantsScreenshot = false;
            this.takeScreenshot();
        }
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
        this.instancedArraysExt = unwrapNull(this.gl.getExtension('ANGLE_instanced_arrays'));
        this.textureHalfFloatExt = unwrapNull(this.gl.getExtension('OES_texture_half_float'));
        this.timerQueryExt = this.gl.getExtension('EXT_disjoint_timer_query');
        this.vertexArrayObjectExt = unwrapNull(this.gl.getExtension('OES_vertex_array_object'));
        // this.gl.getExtension('EXT_frag_depth');
        this.gl.getExtension('OES_element_index_uint');
        this.gl.getExtension('OES_standard_derivatives');
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

    readonly colorAlphaFormat: ColorAlphaFormat;

    readonly shaderPrograms: ShaderMap<PathfinderShaderProgram>;
    readonly gammaLUT: HTMLImageElement;

    readonly quadPositionsBuffer: WebGLBuffer;
    readonly quadElementsBuffer: WebGLBuffer;

    initQuadVAO(attributes: any): void;
    setDirty(): void;
}
