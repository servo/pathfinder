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
import {PathfinderError, expectNotNull, unwrapNull} from './utils';

export interface UnlinkedShaderProgram {
    vertex: WebGLShader;
    fragment: WebGLShader;
}

const COMMON_SHADER_URL: string = '/glsl/gles2/common.inc.glsl';

export const SHADER_NAMES: Array<keyof ShaderMap<void>> = [
    'blit',
    'directCurve',
    'directInterior',
    'ecaaEdgeDetect',
    'ecaaCover',
    'ecaaLine',
    'ecaaCurve',
    'ecaaMonoResolve',
    'ecaaMultiResolve',
];

const SHADER_URLS: ShaderMap<ShaderProgramURLs> = {
    blit: {
        vertex: "/glsl/gles2/blit.vs.glsl",
        fragment: "/glsl/gles2/blit.fs.glsl",
    },
    directCurve: {
        vertex: "/glsl/gles2/direct-curve.vs.glsl",
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
    },
    directInterior: {
        vertex: "/glsl/gles2/direct-interior.vs.glsl",
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
    },
    ecaaEdgeDetect: {
        vertex: "/glsl/gles2/ecaa-edge-detect.vs.glsl",
        fragment: "/glsl/gles2/ecaa-edge-detect.fs.glsl",
    },
    ecaaCover: {
        vertex: "/glsl/gles2/ecaa-cover.vs.glsl",
        fragment: "/glsl/gles2/ecaa-cover.fs.glsl",
    },
    ecaaLine: {
        vertex: "/glsl/gles2/ecaa-line.vs.glsl",
        fragment: "/glsl/gles2/ecaa-line.fs.glsl",
    },
    ecaaCurve: {
        vertex: "/glsl/gles2/ecaa-curve.vs.glsl",
        fragment: "/glsl/gles2/ecaa-curve.fs.glsl",
    },
    ecaaMonoResolve: {
        vertex: "/glsl/gles2/ecaa-mono-resolve.vs.glsl",
        fragment: "/glsl/gles2/ecaa-mono-resolve.fs.glsl",
    },
    ecaaMultiResolve: {
        vertex: "/glsl/gles2/ecaa-multi-resolve.vs.glsl",
        fragment: "/glsl/gles2/ecaa-multi-resolve.fs.glsl",
    },
};

export interface ShaderMap<T> {
    blit: T;
    directCurve: T;
    directInterior: T;
    ecaaEdgeDetect: T;
    ecaaCover: T;
    ecaaLine: T;
    ecaaCurve: T;
    ecaaMonoResolve: T;
    ecaaMultiResolve: T;
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
    load() {
        this.common = window.fetch(COMMON_SHADER_URL).then(response => response.text());

        const shaderKeys = Object.keys(SHADER_URLS) as Array<keyof ShaderMap<string>>;
        let promises = [];
        for (const shaderKey of shaderKeys) {
            promises.push(Promise.all([
                window.fetch(SHADER_URLS[shaderKey].vertex).then(response => response.text()),
                window.fetch(SHADER_URLS[shaderKey].fragment).then(response => response.text()),
            ]).then(results => { return { vertex: results[0], fragment: results[1] } }));
        }

        this.shaders = Promise.all(promises).then(promises => {
            let shaderMap: Partial<ShaderMap<ShaderProgramSource>> = {};
            for (let keyIndex = 0; keyIndex < shaderKeys.length; keyIndex++)
                shaderMap[shaderKeys[keyIndex]] = promises[keyIndex];
            return shaderMap as ShaderMap<ShaderProgramSource>;
        });
    }

    common: Promise<string>;
    shaders: Promise<ShaderMap<ShaderProgramSource>>;
}

export class PathfinderShaderProgram {
    constructor(gl: WebGLRenderingContext,
                programName: string,
                unlinkedShaderProgram: UnlinkedShaderProgram) {
        this.program = expectNotNull(gl.createProgram(), "Failed to create shader program!");
        for (const compiledShader of Object.values(unlinkedShaderProgram))
            gl.attachShader(this.program, compiledShader);
        gl.linkProgram(this.program);

        if (gl.getProgramParameter(this.program, gl.LINK_STATUS) == 0) {
            const infoLog = gl.getProgramInfoLog(this.program);
            throw new PathfinderError(`Failed to link program "${programName}":\n${infoLog}`);
        }

        const uniformCount = gl.getProgramParameter(this.program, gl.ACTIVE_UNIFORMS);
        const attributeCount = gl.getProgramParameter(this.program, gl.ACTIVE_ATTRIBUTES);

        let uniforms: UniformMap = {};
        let attributes: AttributeMap = {};

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

    readonly uniforms: UniformMap;
    readonly attributes: AttributeMap;
    readonly program: WebGLProgram;
}
