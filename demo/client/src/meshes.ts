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

import * as _ from 'lodash';
import {expectNotNull, FLOAT32_SIZE, panic, PathfinderError, UINT16_SIZE} from './utils';
import {UINT32_MAX, UINT32_SIZE} from './utils';

interface BufferTypeFourCCTable {
    [fourCC: string]: keyof Meshes<void>;
}

interface ArrayLike<T> {
    [index: number]: T;
}

interface VertexExpansionDescriptor<T> {
    expanded: T[];
    original: ArrayLike<T>;
    size: number;
}

interface VertexCopyResult {
    originalStartIndex: number;
    originalEndIndex: number;
    expandedStartIndex: number;
    expandedEndIndex: number;
}

type PrimitiveType = 'Uint16' | 'Uint32' | 'Float32';

type PrimitiveTypeArray = Float32Array | Uint16Array | Uint32Array;

interface MeshBufferTypeDescriptor {
    type: PrimitiveType;
    size: number;
}

const PRIMITIVE_TYPE_ARRAY_CONSTRUCTORS = {
    Float32: Float32Array,
    Uint16: Uint16Array,
    Uint32: Uint32Array,
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

const MESH_TYPES: Meshes<MeshBufferTypeDescriptor> = {
    bQuadNormals: { type: 'Float32', size: 4 },
    bQuads: { type: 'Uint32', size: B_QUAD_FIELD_COUNT },
    bVertexLoopBlinnData: { type: 'Uint32', size: 1 },
    bVertexPathIDs: { type: 'Uint16', size: 1 },
    bVertexPositions: { type: 'Float32', size: 2 },
    coverCurveIndices: { type: 'Uint32', size: 1 },
    coverInteriorIndices: { type: 'Uint32', size: 1 },
    edgeBoundingBoxPathIDs: { type: 'Uint16', size: 1 },
    edgeBoundingBoxVertexPositions: { type: 'Float32', size: 4 },
    edgeLowerCurvePathIDs: { type: 'Uint16', size: 1 },
    edgeLowerCurveVertexPositions: { type: 'Float32', size: 6 },
    edgeLowerLinePathIDs: { type: 'Uint16', size: 1 },
    edgeLowerLineVertexPositions: { type: 'Float32', size: 4 },
    edgeUpperCurvePathIDs: { type: 'Uint16', size: 1 },
    edgeUpperCurveVertexPositions: { type: 'Float32', size: 6 },
    edgeUpperLinePathIDs: { type: 'Uint16', size: 1 },
    edgeUpperLineVertexPositions: { type: 'Float32', size: 4 },
};

const BUFFER_TYPES: Meshes<BufferType> = {
    bQuads: 'ARRAY_BUFFER',
    bQuadNormals: 'ARRAY_BUFFER',
    bVertexLoopBlinnData: 'ARRAY_BUFFER',
    bVertexPathIDs: 'ARRAY_BUFFER',
    bVertexPositions: 'ARRAY_BUFFER',
    coverCurveIndices: 'ELEMENT_ARRAY_BUFFER',
    coverInteriorIndices: 'ELEMENT_ARRAY_BUFFER',
    edgeBoundingBoxPathIDs: 'ARRAY_BUFFER',
    edgeBoundingBoxVertexPositions: 'ARRAY_BUFFER',
    edgeLowerCurvePathIDs: 'ARRAY_BUFFER',
    edgeLowerCurveVertexPositions: 'ARRAY_BUFFER',
    edgeLowerLinePathIDs: 'ARRAY_BUFFER',
    edgeLowerLineVertexPositions: 'ARRAY_BUFFER',
    edgeUpperCurvePathIDs: 'ARRAY_BUFFER',
    edgeUpperCurveVertexPositions: 'ARRAY_BUFFER',
    edgeUpperLinePathIDs: 'ARRAY_BUFFER',
    edgeUpperLineVertexPositions: 'ARRAY_BUFFER',
};

const EDGE_BUFFER_NAMES = ['BoundingBox', 'UpperLine', 'UpperCurve', 'LowerLine', 'LowerCurve'];

const RIFF_FOURCC: string = 'RIFF';

const MESH_LIBRARY_FOURCC: string = 'PFML';

// Must match the FourCCs in `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into()`.
const BUFFER_TYPE_FOURCCS: BufferTypeFourCCTable = {
    bqua: 'bQuads',
    bqno: 'bQuadNormals',
    bvlb: 'bVertexLoopBlinnData',
    bvpi: 'bVertexPathIDs',
    bvpo: 'bVertexPositions',
    cvci: 'coverCurveIndices',
    cvii: 'coverInteriorIndices',
    ebbp: 'edgeBoundingBoxPathIDs',
    ebbv: 'edgeBoundingBoxVertexPositions',
    elcp: 'edgeLowerCurvePathIDs',
    elcv: 'edgeLowerCurveVertexPositions',
    ellp: 'edgeLowerLinePathIDs',
    ellv: 'edgeLowerLineVertexPositions',
    eucp: 'edgeUpperCurvePathIDs',
    eucv: 'edgeUpperCurveVertexPositions',
    eulp: 'edgeUpperLinePathIDs',
    eulv: 'edgeUpperLineVertexPositions',
};

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

export interface Meshes<T> {
    readonly bQuads: T;
    readonly bQuadNormals: T;
    readonly bVertexPositions: T;
    readonly bVertexPathIDs: T;
    readonly bVertexLoopBlinnData: T;
    readonly coverInteriorIndices: T;
    readonly coverCurveIndices: T;
    readonly edgeBoundingBoxPathIDs: T;
    readonly edgeBoundingBoxVertexPositions: T;
    readonly edgeLowerCurvePathIDs: T;
    readonly edgeLowerCurveVertexPositions: T;
    readonly edgeLowerLinePathIDs: T;
    readonly edgeLowerLineVertexPositions: T;
    readonly edgeUpperCurvePathIDs: T;
    readonly edgeUpperCurveVertexPositions: T;
    readonly edgeUpperLinePathIDs: T;
    readonly edgeUpperLineVertexPositions: T;
}

export class PathfinderMeshData implements Meshes<ArrayBuffer> {
    readonly bQuads: ArrayBuffer;
    readonly bQuadNormals: ArrayBuffer;
    readonly bVertexPositions: ArrayBuffer;
    readonly bVertexPathIDs: ArrayBuffer;
    readonly bVertexLoopBlinnData: ArrayBuffer;
    readonly coverInteriorIndices: ArrayBuffer;
    readonly coverCurveIndices: ArrayBuffer;
    readonly edgeBoundingBoxPathIDs: ArrayBuffer;
    readonly edgeBoundingBoxVertexPositions: ArrayBuffer;
    readonly edgeLowerCurvePathIDs: ArrayBuffer;
    readonly edgeLowerCurveVertexPositions: ArrayBuffer;
    readonly edgeLowerLinePathIDs: ArrayBuffer;
    readonly edgeLowerLineVertexPositions: ArrayBuffer;
    readonly edgeUpperCurvePathIDs: ArrayBuffer;
    readonly edgeUpperCurveVertexPositions: ArrayBuffer;
    readonly edgeUpperLinePathIDs: ArrayBuffer;
    readonly edgeUpperLineVertexPositions: ArrayBuffer;

    readonly bQuadCount: number;
    readonly edgeLowerCurveCount: number;
    readonly edgeUpperCurveCount: number;
    readonly edgeLowerLineCount: number;
    readonly edgeUpperLineCount: number;

    constructor(meshes: ArrayBuffer | Meshes<ArrayBuffer>) {
        if (meshes instanceof ArrayBuffer) {
            // RIFF encoded data.
            if (toFourCC(meshes, 0) !== RIFF_FOURCC)
                panic("Supplied array buffer is not a mesh library (no RIFF header)!");
            if (toFourCC(meshes, 8) !== MESH_LIBRARY_FOURCC)
                panic("Supplied array buffer is not a mesh library (no PFML header)!");

            let offset = 12;
            while (offset < meshes.byteLength) {
                const fourCC = toFourCC(meshes, offset);
                const chunkLength = (new Uint32Array(meshes.slice(offset + 4, offset + 8)))[0];
                if (BUFFER_TYPE_FOURCCS.hasOwnProperty(fourCC)) {
                    const startOffset = offset + 8;
                    const endOffset = startOffset + chunkLength;
                    this[BUFFER_TYPE_FOURCCS[fourCC]] = meshes.slice(startOffset, endOffset);
                }
                offset += chunkLength + 8;
            }
        } else {
            for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>)
                this[bufferName] = meshes[bufferName];
        }

        this.bQuadCount = this.bQuads.byteLength / B_QUAD_SIZE;
        this.edgeUpperLineCount = this.edgeUpperLinePathIDs.byteLength / 2;
        this.edgeLowerLineCount = this.edgeLowerLinePathIDs.byteLength / 2;
        this.edgeUpperCurveCount = this.edgeUpperCurvePathIDs.byteLength / 2;
        this.edgeLowerCurveCount = this.edgeLowerCurvePathIDs.byteLength / 2;
    }

    expand(pathIDs: number[]): PathfinderMeshData {
        const tempOriginalBuffers: any = {}, tempExpandedArrays: any = {};
        for (const key of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>) {
            const arrayConstructor = PRIMITIVE_TYPE_ARRAY_CONSTRUCTORS[MESH_TYPES[key].type];
            tempOriginalBuffers[key] = new arrayConstructor(this[key]);
            tempExpandedArrays[key] = [];
        }

        const originalBuffers: Meshes<PrimitiveTypeArray> = tempOriginalBuffers;
        const expandedArrays: Meshes<number[]> = tempExpandedArrays;

        for (let newPathIndex = 0; newPathIndex < pathIDs.length; newPathIndex++) {
            const expandedPathID = newPathIndex + 1;
            const originalPathID = pathIDs[newPathIndex];

            const bVertexCopyResult =
                copyVertices(['bVertexPositions', 'bVertexLoopBlinnData'],
                             'bVertexPathIDs',
                             expandedArrays,
                             originalBuffers,
                             expandedPathID,
                             originalPathID);

            if (bVertexCopyResult == null)
                continue;

            const firstExpandedBVertexIndex = bVertexCopyResult.expandedStartIndex;
            const firstBVertexIndex = bVertexCopyResult.originalStartIndex;
            const lastBVertexIndex = bVertexCopyResult.originalEndIndex;

            // Copy over edge data.
            for (const edgeBufferName of EDGE_BUFFER_NAMES) {
                copyVertices([`edge${edgeBufferName}VertexPositions` as keyof Meshes<void>],
                             `edge${edgeBufferName}PathIDs` as keyof Meshes<void>,
                             expandedArrays,
                             originalBuffers,
                             expandedPathID,
                             originalPathID);
            }

            // Copy over indices.
            copyIndices(expandedArrays.coverInteriorIndices,
                        originalBuffers.coverInteriorIndices as Uint32Array,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        lastBVertexIndex);
            copyIndices(expandedArrays.coverCurveIndices,
                        originalBuffers.coverCurveIndices as Uint32Array,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        lastBVertexIndex);

            // Copy over B-quads.
            let firstBQuadIndex =
                findFirstBQuadIndex(originalBuffers.bQuads as Uint32Array,
                                    originalBuffers.bVertexPathIDs as Uint16Array,
                                    originalPathID);
            if (firstBQuadIndex == null)
                firstBQuadIndex = originalBuffers.bQuads.length;
            const indexDelta = firstExpandedBVertexIndex - firstBVertexIndex;
            for (let bQuadIndex = firstBQuadIndex;
                 bQuadIndex < originalBuffers.bQuads.length / B_QUAD_FIELD_COUNT;
                 bQuadIndex++) {
                const bQuad = originalBuffers.bQuads[bQuadIndex];
                if (originalBuffers.bVertexPathIDs[originalBuffers.bQuads[bQuadIndex *
                                                                          B_QUAD_FIELD_COUNT]] !==
                                                                          originalPathID) {
                    break;
                }

                for (let indexIndex = 0; indexIndex < B_QUAD_FIELD_COUNT; indexIndex++) {
                    const srcIndex = originalBuffers.bQuads[bQuadIndex * B_QUAD_FIELD_COUNT +
                                                            indexIndex];
                    if (srcIndex === UINT32_MAX)
                        expandedArrays.bQuads.push(srcIndex);
                    else
                        expandedArrays.bQuads.push(srcIndex + indexDelta);
                }

                for (let angleIndex = 0; angleIndex < 4; angleIndex++) {
                    const srcAngle = originalBuffers.bQuadNormals[bQuadIndex * 4 + angleIndex];
                    expandedArrays.bQuadNormals.push(srcAngle);
                }
            }
        }

        const tempExpandedBuffers: any = {};
        for (const key of Object.keys(MESH_TYPES) as Array<keyof Meshes<void>>) {
            const bufferType = MESH_TYPES[key].type;
            const arrayConstructor = PRIMITIVE_TYPE_ARRAY_CONSTRUCTORS[bufferType];
            const expandedBuffer = new ArrayBuffer(expandedArrays[key].length *
                                                   sizeOfPrimitive(bufferType));
            (new arrayConstructor(expandedBuffer)).set(expandedArrays[key]);
            tempExpandedBuffers[key] = expandedBuffer;
        }

        const expandedBuffers = tempExpandedBuffers as Meshes<ArrayBuffer>;
        return new PathfinderMeshData(expandedBuffers);
    }
}

export class PathfinderMeshBuffers implements Meshes<WebGLBuffer> {
    readonly bQuads: WebGLBuffer;
    readonly bQuadNormals: WebGLBuffer;
    readonly bVertexPositions: WebGLBuffer;
    readonly bVertexPathIDs: WebGLBuffer;
    readonly bVertexLoopBlinnData: WebGLBuffer;
    readonly coverInteriorIndices: WebGLBuffer;
    readonly coverCurveIndices: WebGLBuffer;
    readonly edgeBoundingBoxPathIDs: WebGLBuffer;
    readonly edgeBoundingBoxVertexPositions: WebGLBuffer;
    readonly edgeLowerCurvePathIDs: WebGLBuffer;
    readonly edgeLowerCurveVertexPositions: WebGLBuffer;
    readonly edgeLowerLinePathIDs: WebGLBuffer;
    readonly edgeLowerLineVertexPositions: WebGLBuffer;
    readonly edgeUpperCurvePathIDs: WebGLBuffer;
    readonly edgeUpperCurveVertexPositions: WebGLBuffer;
    readonly edgeUpperLinePathIDs: WebGLBuffer;
    readonly edgeUpperLineVertexPositions: WebGLBuffer;

    constructor(gl: WebGLRenderingContext, meshData: PathfinderMeshData) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof PathfinderMeshBuffers>) {
            const bufferType = gl[BUFFER_TYPES[bufferName]];
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
            this[bufferName] = buffer;
        }
    }
}

function copyVertices<T>(vertexBufferNames: Array<keyof Meshes<void>>,
                         pathIDBufferName: keyof Meshes<void>,
                         expandedMeshes: Meshes<number[]>,
                         originalMeshes: Meshes<PrimitiveTypeArray>,
                         expandedPathID: number,
                         originalPathID: number):
                         VertexCopyResult | null {
    const expandedPathIDs = expandedMeshes[pathIDBufferName];
    const originalPathIDs = originalMeshes[pathIDBufferName];

    const firstOriginalVertexIndex = _.sortedIndex(originalPathIDs, originalPathID);
    if (firstOriginalVertexIndex < 0)
        return null;

    const firstExpandedVertexIndex = expandedPathIDs.length;
    let lastOriginalVertexIndex = firstOriginalVertexIndex;

    while (lastOriginalVertexIndex < originalPathIDs.length &&
           originalPathIDs[lastOriginalVertexIndex] === originalPathID) {
        for (const vertexBufferName of vertexBufferNames) {
            const expanded = expandedMeshes[vertexBufferName];
            const original = originalMeshes[vertexBufferName];
            const size = MESH_TYPES[vertexBufferName].size;
            for (let elementIndex = 0; elementIndex < size; elementIndex++) {
                const globalIndex = size * lastOriginalVertexIndex + elementIndex;
                expanded.push(original[globalIndex]);
            }
        }

        expandedPathIDs.push(expandedPathID);

        lastOriginalVertexIndex++;
    }

    return {
        expandedEndIndex: expandedPathIDs.length,
        expandedStartIndex: firstExpandedVertexIndex,
        originalEndIndex: lastOriginalVertexIndex,
        originalStartIndex: firstOriginalVertexIndex,
    };
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

function sizeOfPrimitive(primitiveType: PrimitiveType): number {
    switch (primitiveType) {
    case 'Uint16':  return UINT16_SIZE;
    case 'Uint32':  return UINT32_SIZE;
    case 'Float32': return FLOAT32_SIZE;
    }
}

function findFirstBQuadIndex(bQuads: Uint32Array,
                             bVertexPathIDs: Uint16Array,
                             queryPathID: number):
                             number | null {
    for (let bQuadIndex = 0; bQuadIndex < bQuads.length / B_QUAD_FIELD_COUNT; bQuadIndex++) {
        const thisPathID = bVertexPathIDs[bQuads[bQuadIndex * B_QUAD_FIELD_COUNT]];
        if (thisPathID === queryPathID)
            return bQuadIndex;
    }
    return null;
}

function toFourCC(buffer: ArrayBuffer, position: number): string {
    let result = "";
    const bytes = new Uint8Array(buffer, position, 4);
    for (const byte of bytes)
        result += String.fromCharCode(byte);
    return result;
}

export function parseServerTiming(headers: Headers): number {
    if (!headers.has('Server-Timing'))
        return 0.0;
    const timing = headers.get('Server-Timing')!;
    const matches = /^Partitioning\s*=\s*([0-9.]+)$/.exec(timing);
    return matches != null ? parseFloat(matches[1]) / 1000.0 : 0.0;
}
