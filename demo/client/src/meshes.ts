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

import { PathfinderError, expectNotNull, panic, UINT32_SIZE, UINT32_MAX } from './utils';
import * as _ from 'lodash';

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

const B_QUAD_FIELD_COUNT: number = B_QUAD_SIZE / UINT32_SIZE;

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
    constructor(meshes: Meshes<string | ArrayBuffer>) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>) {
            const meshBuffer = meshes[bufferName];
            if (typeof(meshBuffer) === 'string')
                this[bufferName] = base64js.toByteArray(meshBuffer).buffer as ArrayBuffer;
            else if (meshBuffer instanceof ArrayBuffer)
                this[bufferName] = meshBuffer;
            else
                panic("Unknown buffer type!");
        }

        this.bQuadCount = this.bQuads.byteLength / B_QUAD_SIZE;
        this.edgeUpperLineIndexCount = this.edgeUpperLineIndices.byteLength / 8;
        this.edgeLowerLineIndexCount = this.edgeLowerLineIndices.byteLength / 8;
        this.edgeUpperCurveIndexCount = this.edgeUpperCurveIndices.byteLength / 16;
        this.edgeLowerCurveIndexCount = this.edgeLowerCurveIndices.byteLength / 16;
    }

    expand(pathIDs: number[]): PathfinderMeshData {
        const bQuads = new Uint32Array(this.bQuads);
        const bVertexPositions = new Float32Array(this.bVertexPositions);
        const bVertexPathIDs = new Uint16Array(this.bVertexPathIDs);
        const bVertexLoopBlinnData = new Uint32Array(this.bVertexLoopBlinnData);

        const edgeUpperCurveIndices = new Uint32Array(this.edgeUpperCurveIndices);
        const edgeLowerCurveIndices = new Uint32Array(this.edgeLowerCurveIndices);
        for (let indexIndex = 3; indexIndex < edgeUpperCurveIndices.length; indexIndex += 4)
            edgeUpperCurveIndices[indexIndex] = 0;
        for (let indexIndex = 3; indexIndex < edgeLowerCurveIndices.length; indexIndex += 4)
            edgeLowerCurveIndices[indexIndex] = 0;

        const coverInteriorIndices = new Uint32Array(this.coverInteriorIndices);
        const coverCurveIndices = new Uint32Array(this.coverCurveIndices);
        const edgeUpperLineIndices = new Uint32Array(this.edgeUpperLineIndices);
        const edgeLowerLineIndices = new Uint32Array(this.edgeLowerLineIndices);

        const expandedBQuads: number[] = [];
        const expandedBVertexPositions: number[] = [];
        const expandedBVertexPathIDs: number[] = [];
        const expandedBVertexLoopBlinnData: number[] = [];
        const expandedCoverInteriorIndices: number[] = [];
        const expandedCoverCurveIndices: number[] = [];
        const expandedEdgeUpperCurveIndices: number[] = [];
        const expandedEdgeUpperLineIndices: number[] = [];
        const expandedEdgeLowerCurveIndices: number[] = [];
        const expandedEdgeLowerLineIndices: number[] = [];

        let textGlyphIndex = 0;
        for (const pathID of pathIDs) {
            const firstBVertexIndex = _.sortedIndex(bVertexPathIDs, pathID);
            if (firstBVertexIndex < 0)
                continue;

            // Copy over vertices.
            let bVertexIndex = firstBVertexIndex;
            const firstExpandedBVertexIndex = expandedBVertexPathIDs.length;
            while (bVertexIndex < bVertexPathIDs.length &&
                bVertexPathIDs[bVertexIndex] === pathID) {
                expandedBVertexPositions.push(bVertexPositions[bVertexIndex * 2 + 0],
                                            bVertexPositions[bVertexIndex * 2 + 1]);
                expandedBVertexPathIDs.push(textGlyphIndex + 1);
                expandedBVertexLoopBlinnData.push(bVertexLoopBlinnData[bVertexIndex]);
                bVertexIndex++;
            }

            // Copy over indices.
            copyIndices(expandedCoverInteriorIndices,
                        coverInteriorIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex);
            copyIndices(expandedCoverCurveIndices,
                        coverCurveIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex);

            copyIndices(expandedEdgeUpperLineIndices,
                        edgeUpperLineIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex);
            copyIndices(expandedEdgeUpperCurveIndices,
                        edgeUpperCurveIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex,
                        indexIndex => indexIndex % 4 < 3);
            copyIndices(expandedEdgeLowerLineIndices,
                        edgeLowerLineIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex);
            copyIndices(expandedEdgeLowerCurveIndices,
                        edgeLowerCurveIndices,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        bVertexIndex,
                        indexIndex => indexIndex % 4 < 3);

            // Copy over B-quads.
            let firstBQuadIndex = findFirstBQuadIndex(bQuads, pathID);
            if (firstBQuadIndex == null)
                firstBQuadIndex = bQuads.length;
            const indexDelta = firstExpandedBVertexIndex - firstBVertexIndex;
            for (let bQuadIndex = firstBQuadIndex; bQuadIndex < bQuads.length; bQuadIndex++) {
                const bQuad = bQuads[bQuadIndex];
                if (bVertexPathIDs[bQuads[bQuadIndex * B_QUAD_FIELD_COUNT]] !== pathID)
                    break;
                for (let indexIndex = 0; indexIndex < B_QUAD_FIELD_COUNT; indexIndex++) {
                    const srcIndex = bQuads[bQuadIndex * B_QUAD_FIELD_COUNT + indexIndex];
                    if (srcIndex === UINT32_MAX)
                        expandedBQuads.push(srcIndex);
                    else
                        expandedBQuads.push(srcIndex + indexDelta);
                }
            }

            textGlyphIndex++;
        }

        return new PathfinderMeshData({
            bQuads: new Uint32Array(expandedBQuads).buffer as ArrayBuffer,
            bVertexPositions: new Float32Array(expandedBVertexPositions).buffer as ArrayBuffer,
            bVertexPathIDs: new Uint16Array(expandedBVertexPathIDs).buffer as ArrayBuffer,
            bVertexLoopBlinnData: new Uint32Array(expandedBVertexLoopBlinnData).buffer as
                ArrayBuffer,
            coverInteriorIndices: new Uint32Array(expandedCoverInteriorIndices).buffer as
                ArrayBuffer,
            coverCurveIndices: new Uint32Array(expandedCoverCurveIndices).buffer as ArrayBuffer,
            edgeUpperCurveIndices: new Uint32Array(expandedEdgeUpperCurveIndices).buffer as
                ArrayBuffer,
            edgeUpperLineIndices: new Uint32Array(expandedEdgeUpperLineIndices).buffer as
                ArrayBuffer,
            edgeLowerCurveIndices: new Uint32Array(expandedEdgeLowerCurveIndices).buffer as
                ArrayBuffer,
            edgeLowerLineIndices: new Uint32Array(expandedEdgeLowerLineIndices).buffer as
                ArrayBuffer,
        })
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

function copyIndices(destIndices: number[],
                     srcIndices: Uint32Array,
                     firstExpandedIndex: number,
                     firstIndex: number,
                     lastIndex: number,
                     validateIndex?: (indexIndex: number) => boolean) {
    if (firstIndex === lastIndex)
        return;

    // FIXME(pcwalton): Speed this up somehow.
    let indexIndex = srcIndices.findIndex(index => index >= firstIndex && index < lastIndex);
    if (indexIndex < 0)
        return;

    const indexDelta = firstExpandedIndex - firstIndex;
    while (indexIndex < srcIndices.length) {
        const index = srcIndices[indexIndex];
        if (validateIndex == null || validateIndex(indexIndex)) {
            if (index < firstIndex || index >= lastIndex)
                break;
            destIndices.push(index + indexDelta);
        } else {
            destIndices.push(index);
        }
        indexIndex++;
    }
}

function findFirstBQuadIndex(bQuads: Uint32Array, queryPathID: number): number | null {
    let low = 0, high = bQuads.length / B_QUAD_FIELD_COUNT;
    while (low < high) {
        const mid = low + (high - low) / 2;
        const thisPathID = bQuads[mid * B_QUAD_FIELD_COUNT];
        if (queryPathID <= thisPathID)
            high = mid;
        else 
            low = mid + 1;
    }
    return bQuads[low * B_QUAD_FIELD_COUNT] === queryPathID ? low : null;
}
