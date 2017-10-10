// pathfinder/client/src/shader-loader.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AttributeMap, UniformMap} from './gl-utils';
import {expectNotNull, PathfinderError, unwrapNull} from './utils';

export interface UnlinkedShaderProgram {
    vertex: WebGLShader;
    fragment: WebGLShader;
}

const COMMON_SHADER_URL: string = '/glsl/gles2/common.inc.glsl';

export const SHADER_NAMES: Array<keyof ShaderMap<void>> = [
    'blit',
    'directCurve',
    'directInterior',
    'direct3DCurve',
    'direct3DInterior',
    'ssaaSubpixelResolve',
    'ecaaEdgeDetect',
    'ecaaFastCover',
    'ecaaFastLine',
    'ecaaFastCurve',
    'ecaaBoldLine',
    'ecaaBoldCurve',
    'ecaaMonoResolve',
    'ecaaMonoSubpixelResolve',
    'ecaaMultiResolve',
    'demo3DMonument',
];

const SHADER_URLS: ShaderMap<ShaderProgramURLs> = {
    blit: {
        fragment: "/glsl/gles2/blit.fs.glsl",
        vertex: "/glsl/gles2/blit.vs.glsl",
    },
    demo3DMonument: {
        fragment: "/glsl/gles2/demo-3d-monument.fs.glsl",
        vertex: "/glsl/gles2/demo-3d-monument.vs.glsl",
    },
    direct3DCurve: {
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
        vertex: "/glsl/gles2/direct-3d-curve.vs.glsl",
    },
    direct3DInterior: {
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
        vertex: "/glsl/gles2/direct-3d-interior.vs.glsl",
    },
    directCurve: {
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
        vertex: "/glsl/gles2/direct-curve.vs.glsl",
    },
    directInterior: {
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
        vertex: "/glsl/gles2/direct-interior.vs.glsl",
    },
    ecaaBoldCurve: {
        fragment: "/glsl/gles2/ecaa-curve.fs.glsl",
        vertex: "/glsl/gles2/ecaa-bold-curve.vs.glsl",
    },
    ecaaBoldLine: {
        fragment: "/glsl/gles2/ecaa-line.fs.glsl",
        vertex: "/glsl/gles2/ecaa-bold-line.vs.glsl",
    },
    ecaaEdgeDetect: {
        fragment: "/glsl/gles2/ecaa-edge-detect.fs.glsl",
        vertex: "/glsl/gles2/ecaa-edge-detect.vs.glsl",
    },
    ecaaFastCover: {
        fragment: "/glsl/gles2/ecaa-fast-cover.fs.glsl",
        vertex: "/glsl/gles2/ecaa-fast-cover.vs.glsl",
    },
    ecaaFastCurve: {
        fragment: "/glsl/gles2/ecaa-curve.fs.glsl",
        vertex: "/glsl/gles2/ecaa-fast-curve.vs.glsl",
    },
    ecaaFastLine: {
        fragment: "/glsl/gles2/ecaa-line.fs.glsl",
        vertex: "/glsl/gles2/ecaa-fast-line.vs.glsl",
    },
    ecaaMonoResolve: {
        fragment: "/glsl/gles2/ecaa-mono-resolve.fs.glsl",
        vertex: "/glsl/gles2/ecaa-mono-resolve.vs.glsl",
    },
    ecaaMonoSubpixelResolve: {
        fragment: "/glsl/gles2/ecaa-mono-subpixel-resolve.fs.glsl",
        vertex: "/glsl/gles2/ecaa-mono-subpixel-resolve.vs.glsl",
    },
    ecaaMultiResolve: {
        fragment: "/glsl/gles2/ecaa-multi-resolve.fs.glsl",
        vertex: "/glsl/gles2/ecaa-multi-resolve.vs.glsl",
    },
    ssaaSubpixelResolve: {
        fragment: "/glsl/gles2/ssaa-subpixel-resolve.fs.glsl",
        vertex: "/glsl/gles2/ssaa-subpixel-resolve.vs.glsl",
    },
};

export interface ShaderMap<T> {
    blit: T;
    directCurve: T;
    directInterior: T;
    direct3DCurve: T;
    direct3DInterior: T;
    ssaaSubpixelResolve: T;
    ecaaEdgeDetect: T;
    ecaaFastCover: T;
    ecaaFastLine: T;
    ecaaFastCurve: T;
    ecaaBoldLine: T;
    ecaaBoldCurve: T;
    ecaaMonoResolve: T;
    ecaaMonoSubpixelResolve: T;
    ecaaMultiResolve: T;
    demo3DMonument: T;
}

export interface ShaderProgramSource {
    vertex: string;
    fragment: string;
}

interface ShaderProgramURLs {
    vertex: string;
    fragment: string;
}

export class ShaderLoader {
    common: Promise<string>;
    shaders: Promise<ShaderMap<ShaderProgramSource>>;

    load() {
        this.common = window.fetch(COMMON_SHADER_URL).then(response => response.text());

        const shaderKeys = Object.keys(SHADER_URLS) as Array<keyof ShaderMap<string>>;
        const promises = [];
        for (const shaderKey of shaderKeys) {
            promises.push(Promise.all([
                window.fetch(SHADER_URLS[shaderKey].vertex).then(response => response.text()),
                window.fetch(SHADER_URLS[shaderKey].fragment).then(response => response.text()),
            ]).then(results => ({ vertex: results[0], fragment: results[1] })));
        }

        this.shaders = Promise.all(promises).then(promises => {
            const shaderMap: Partial<ShaderMap<ShaderProgramSource>> = {};
            for (let keyIndex = 0; keyIndex < shaderKeys.length; keyIndex++)
                shaderMap[shaderKeys[keyIndex]] = promises[keyIndex];
            return shaderMap as ShaderMap<ShaderProgramSource>;
        });
    }
}

export class PathfinderShaderProgram {
    readonly uniforms: UniformMap;
    readonly attributes: AttributeMap;
    readonly program: WebGLProgram;

    constructor(gl: WebGLRenderingContext,
                programName: string,
                unlinkedShaderProgram: UnlinkedShaderProgram) {
        this.program = expectNotNull(gl.createProgram(), "Failed to create shader program!");
        for (const compiledShader of Object.values(unlinkedShaderProgram))
            gl.attachShader(this.program, compiledShader);
        gl.linkProgram(this.program);

        if (gl.getProgramParameter(this.program, gl.LINK_STATUS) === 0) {
            const infoLog = gl.getProgramInfoLog(this.program);
            throw new PathfinderError(`Failed to link program "${programName}":\n${infoLog}`);
        }

        const uniformCount = gl.getProgramParameter(this.program, gl.ACTIVE_UNIFORMS);
        const attributeCount = gl.getProgramParameter(this.program, gl.ACTIVE_ATTRIBUTES);

        const uniforms: UniformMap = {};
        const attributes: AttributeMap = {};

        for (let uniformIndex = 0; uniformIndex < uniformCount; uniformIndex++) {
            const uniformName = unwrapNull(gl.getActiveUniform(this.program, uniformIndex)).name;
            uniforms[uniformName] = expectNotNull(gl.getUniformLocation(this.program, uniformName),
                                                  `Didn't find uniform "${uniformName}"!`);
        }
        for (let attributeIndex = 0; attributeIndex < attributeCount; attributeIndex++) {
            const attributeName = unwrapNull(gl.getActiveAttrib(this.program, attributeIndex)).name;
            attributes[attributeName] = attributeIndex;
        }

        this.uniforms = uniforms;
        this.attributes = attributes;
    }
}
