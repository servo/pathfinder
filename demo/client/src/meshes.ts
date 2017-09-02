// pathfinder/client/src/meshes.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as base64js from 'base64-js';

import {PathfinderError, expectNotNull} from './utils';

const BUFFER_TYPES: Meshes<BufferType> = {
    bQuads: 'ARRAY_BUFFER',
    bVertexPositions: 'ARRAY_BUFFER',
    bVertexPathIDs: 'ARRAY_BUFFER',
    bVertexLoopBlinnData: 'ARRAY_BUFFER',
    coverInteriorIndices: 'ELEMENT_ARRAY_BUFFER',
    coverCurveIndices: 'ELEMENT_ARRAY_BUFFER',
    edgeUpperLineIndices: 'ARRAY_BUFFER',
    edgeLowerLineIndices: 'ARRAY_BUFFER',
    edgeUpperCurveIndices: 'ARRAY_BUFFER',
    edgeLowerCurveIndices: 'ARRAY_BUFFER',
};

export const B_QUAD_SIZE: number = 4 * 8;
export const B_QUAD_UPPER_LEFT_VERTEX_OFFSET: number = 4 * 0;
export const B_QUAD_UPPER_RIGHT_VERTEX_OFFSET: number = 4 * 1;
export const B_QUAD_UPPER_CONTROL_POINT_VERTEX_OFFSET: number = 4 * 2;
export const B_QUAD_LOWER_LEFT_VERTEX_OFFSET: number = 4 * 4;
export const B_QUAD_LOWER_RIGHT_VERTEX_OFFSET: number = 4 * 5;
export const B_QUAD_LOWER_CONTROL_POINT_VERTEX_OFFSET: number = 4 * 6;
export const B_QUAD_UPPER_INDICES_OFFSET: number = B_QUAD_UPPER_LEFT_VERTEX_OFFSET;
export const B_QUAD_LOWER_INDICES_OFFSET: number = B_QUAD_LOWER_LEFT_VERTEX_OFFSET;

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

export interface Meshes<T> {
    readonly bQuads: T;
    readonly bVertexPositions: T;
    readonly bVertexPathIDs: T;
    readonly bVertexLoopBlinnData: T;
    readonly coverInteriorIndices: T;
    readonly coverCurveIndices: T;
    readonly edgeUpperLineIndices: T;
    readonly edgeLowerLineIndices: T;
    readonly edgeUpperCurveIndices: T;
    readonly edgeLowerCurveIndices: T;
}

export class PathfinderMeshData implements Meshes<ArrayBuffer> {
    constructor(meshes: any) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>)
            this[bufferName] = base64js.toByteArray(meshes[bufferName]).buffer as ArrayBuffer;

        this.bQuadCount = this.bQuads.byteLength / B_QUAD_SIZE;
        this.edgeUpperLineIndexCount = this.edgeUpperLineIndices.byteLength / 8;
        this.edgeLowerLineIndexCount = this.edgeLowerLineIndices.byteLength / 8;
        this.edgeUpperCurveIndexCount = this.edgeUpperCurveIndices.byteLength / 16;
        this.edgeLowerCurveIndexCount = this.edgeLowerCurveIndices.byteLength / 16;
    }

    readonly bQuads: ArrayBuffer;
    readonly bVertexPositions: ArrayBuffer;
    readonly bVertexPathIDs: ArrayBuffer;
    readonly bVertexLoopBlinnData: ArrayBuffer;
    readonly coverInteriorIndices: ArrayBuffer;
    readonly coverCurveIndices: ArrayBuffer;
    readonly edgeUpperLineIndices: ArrayBuffer;
    readonly edgeLowerLineIndices: ArrayBuffer;
    readonly edgeUpperCurveIndices: ArrayBuffer;
    readonly edgeLowerCurveIndices: ArrayBuffer;

    readonly bQuadCount: number;
    readonly edgeUpperLineIndexCount: number;
    readonly edgeLowerLineIndexCount: number;
    readonly edgeUpperCurveIndexCount: number;
    readonly edgeLowerCurveIndexCount: number;
}

export class PathfinderMeshBuffers implements Meshes<WebGLBuffer> {
    constructor(gl: WebGLRenderingContext, meshData: PathfinderMeshData) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof PathfinderMeshBuffers>) {
            const bufferType = gl[BUFFER_TYPES[bufferName]];
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
            this[bufferName] = buffer;
        }
    }

    readonly bQuads: WebGLBuffer;
    readonly bVertexPositions: WebGLBuffer;
    readonly bVertexPathIDs: WebGLBuffer;
    readonly bVertexLoopBlinnData: WebGLBuffer;
    readonly coverInteriorIndices: WebGLBuffer;
    readonly coverCurveIndices: WebGLBuffer;
    readonly edgeUpperLineIndices: WebGLBuffer;
    readonly edgeUpperCurveIndices: WebGLBuffer;
    readonly edgeLowerLineIndices: WebGLBuffer;
    readonly edgeLowerCurveIndices: WebGLBuffer;
}
