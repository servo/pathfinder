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

import {QUAD_ELEMENTS, UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {PathfinderShaderProgram, SHADER_NAMES, ShaderMap} from './shader-loader';
import {ShaderProgramSource, UnlinkedShaderProgram} from './shader-loader';
import {PathfinderError, expectNotNull, unwrapNull} from './utils';
import PathfinderBufferTexture from './buffer-texture';

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

export abstract class PathfinderView {
    constructor(canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        this.canvas = canvas;

        this.initContext();

        const shaderSource = this.compileShaders(commonShaderSource, shaderSources);
        this.shaderPrograms = this.linkShaders(shaderSource);

        this.atlasTransformBuffer = new PathfinderBufferTexture(this.gl, 'uPathTransform');
        this.pathColorsBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathColors');
    }

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

    abstract setTransformAndTexScaleUniformsForDest(uniforms: UniformMap): void;
    abstract setTransformSTAndTexScaleUniformsForDest(uniforms: UniformMap): void;

    abstract get destFramebuffer(): WebGLFramebuffer;
    abstract get destDepthTexture(): WebGLTexture;

    abstract get destAllocatedSize(): glmatrix.vec2;
    abstract get destUsedSize(): glmatrix.vec2;

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

    atlasTransformBuffer: PathfinderBufferTexture;
    protected pathColorsBufferTexture: PathfinderBufferTexture;
}

export abstract class MonochromePathfinderView extends PathfinderView {
    abstract get bgColor(): glmatrix.vec4;
    abstract get fgColor(): glmatrix.vec4;
}
