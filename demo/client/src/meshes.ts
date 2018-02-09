// pathfinder/client/src/meshes.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as base64js from 'base64-js';

import * as _ from 'lodash';
import {expectNotNull, FLOAT32_SIZE, panic, PathfinderError, Range, UINT16_SIZE} from './utils';
import {UINT32_MAX, UINT32_SIZE, UINT8_SIZE, unwrapNull, unwrapUndef} from './utils';

interface BufferTypeFourCCTable {
    [fourCC: string]: keyof Meshes<void>;
}

interface PathRangeTypeFourCCTable {
    [fourCC: string]: keyof PathRanges;
}

interface RangeToCountTable {
    [rangeKey: string]: keyof MeshDataCounts;
}

interface RangeToRangeBufferTable {
    [rangeKey: string]: keyof Meshes<void>;
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

// FIXME(pcwalton): This duplicates information below in `MESH_TYPES`.
const INDEX_SIZE: number = 4;
const B_QUAD_VERTEX_POSITION_SIZE: number = 12 * 4;
const B_VERTEX_POSITION_SIZE: number = 4 * 2;
const EDGE_BOUNDING_BOX_VERTEX_POSITION_SIZE: number = 4 * 4;
const EDGE_UPPER_LINE_VERTEX_POSITION_SIZE: number = 4 * 4;
const EDGE_LOWER_LINE_VERTEX_POSITION_SIZE: number = 4 * 4;
const EDGE_UPPER_CURVE_VERTEX_POSITION_SIZE: number = 4 * 6;
const EDGE_LOWER_CURVE_VERTEX_POSITION_SIZE: number = 4 * 6;
const SEGMENT_LINE_SIZE: number = 4 * 4;
const SEGMENT_CURVE_SIZE: number = 4 * 6;

const MESH_TYPES: Meshes<MeshBufferTypeDescriptor> = {
    bBoxPathIDs: { type: 'Uint16', size: 1 },
    bBoxes: { type: 'Float32', size: 20 },
    bQuadVertexInteriorIndices: { type: 'Uint32', size: 1 },
    bQuadVertexPositionPathIDs: { type: 'Uint16', size: 6 },
    bQuadVertexPositions: { type: 'Float32', size: 12 },
    stencilNormals: { type: 'Float32', size: 6 },
    stencilSegmentPathIDs: { type: 'Uint16', size: 1 },
    stencilSegments: { type: 'Float32', size: 6 },
};

const BUFFER_TYPES: Meshes<BufferType> = {
    bBoxPathIDs: 'ARRAY_BUFFER',
    bBoxes: 'ARRAY_BUFFER',
    bQuadVertexInteriorIndices: 'ELEMENT_ARRAY_BUFFER',
    bQuadVertexPositionPathIDs: 'ARRAY_BUFFER',
    bQuadVertexPositions: 'ARRAY_BUFFER',
    stencilNormals: 'ARRAY_BUFFER',
    stencilSegmentPathIDs: 'ARRAY_BUFFER',
    stencilSegments: 'ARRAY_BUFFER',
};

const EDGE_BUFFER_NAMES = ['UpperLine', 'UpperCurve', 'LowerLine', 'LowerCurve'];

const RIFF_FOURCC: string = 'RIFF';

const MESH_LIBRARY_FOURCC: string = 'PFML';

// Must match the FourCCs in `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into()`.
const BUFFER_TYPE_FOURCCS: BufferTypeFourCCTable = {
    bbox: 'bBoxes',
    bqii: 'bQuadVertexInteriorIndices',
    bqvp: 'bQuadVertexPositions',
    snor: 'stencilNormals',
    sseg: 'stencilSegments',
};

// Must match the FourCCs in
// `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into::write_path_ranges()`.
const PATH_RANGE_TYPE_FOURCCS: PathRangeTypeFourCCTable = {
    bbox: 'bBoxPathRanges',
    bqii: 'bQuadVertexInteriorIndexPathRanges',
    bqvp: 'bQuadVertexPositionPathRanges',
    sseg: 'stencilSegmentPathRanges',
};

const RANGE_TO_COUNT_TABLE: RangeToCountTable = {
    bBoxPathRanges: 'bBoxCount',
    bQuadVertexInteriorIndexPathRanges: 'bQuadVertexInteriorIndexCount',
    bQuadVertexPositionPathRanges: 'bQuadVertexPositionCount',
    stencilSegmentPathRanges: 'stencilSegmentCount',
};

const RANGE_TO_RANGE_BUFFER_TABLE: RangeToRangeBufferTable = {
    bBoxPathRanges: 'bBoxPathIDs',
    bQuadVertexPositionPathRanges: 'bQuadVertexPositionPathIDs',
    stencilSegmentPathRanges: 'stencilSegmentPathIDs',
};

const RANGE_KEYS: Array<keyof PathRanges> = [
    'bQuadVertexPositionPathRanges',
    'bQuadVertexInteriorIndexPathRanges',
    'bBoxPathRanges',
    'stencilSegmentPathRanges',
];

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

export interface Meshes<T> {
    readonly bQuadVertexPositions: T;
    readonly bQuadVertexInteriorIndices: T;
    readonly bBoxes: T;
    readonly stencilSegments: T;
    readonly stencilNormals: T;

    bQuadVertexPositionPathIDs: T;
    bBoxPathIDs: T;
    stencilSegmentPathIDs: T;
}

interface MeshDataCounts {
    readonly bQuadVertexPositionCount: number;
    readonly bQuadVertexInteriorIndexCount: number;
    readonly bBoxCount: number;
    readonly stencilSegmentCount: number;
}

interface PathRanges {
    bQuadVertexPositionPathRanges: Range[];
    bQuadVertexInteriorIndexPathRanges: Range[];
    bBoxPathRanges: Range[];
    stencilSegmentPathRanges: Range[];
}

export class PathfinderMeshData implements Meshes<ArrayBuffer>, MeshDataCounts, PathRanges {
    readonly bQuadVertexPositions: ArrayBuffer;
    readonly bQuadVertexInteriorIndices: ArrayBuffer;
    readonly bBoxes: ArrayBuffer;
    readonly bBoxSigns: ArrayBuffer;
    readonly bBoxIndices: ArrayBuffer;
    readonly stencilSegments: ArrayBuffer;
    readonly stencilNormals: ArrayBuffer;

    readonly bQuadVertexPositionCount: number;
    readonly bQuadVertexInteriorIndexCount: number;
    readonly bBoxCount: number;
    readonly stencilSegmentCount: number;

    bQuadVertexPositionPathIDs: ArrayBuffer;
    bBoxPathIDs: ArrayBuffer;
    stencilSegmentPathIDs: ArrayBuffer;

    bQuadVertexPositionPathRanges: Range[];
    bQuadVertexInteriorIndexPathRanges: Range[];
    bBoxPathRanges: Range[];
    stencilSegmentPathRanges: Range[];

    constructor(meshes: ArrayBuffer | Meshes<ArrayBuffer>, optionalRanges?: PathRanges) {
        if (meshes instanceof ArrayBuffer) {
            // RIFF encoded data.
            if (toFourCC(meshes, 0) !== RIFF_FOURCC)
                panic("Supplied array buffer is not a mesh library (no RIFF header)!");
            if (toFourCC(meshes, 8) !== MESH_LIBRARY_FOURCC)
                panic("Supplied array buffer is not a mesh library (no PFML header)!");

            let offset = 12;
            while (offset < meshes.byteLength) {
                const fourCC = toFourCC(meshes, offset);
                const chunkLength = readUInt32(meshes, offset + 4);
                const startOffset = offset + 8;
                const endOffset = startOffset + chunkLength;

                if (BUFFER_TYPE_FOURCCS.hasOwnProperty(fourCC))
                    this[BUFFER_TYPE_FOURCCS[fourCC]] = meshes.slice(startOffset, endOffset);
                else if (fourCC === 'prng')
                    this.readPathRanges(meshes.slice(startOffset, endOffset));

                offset = endOffset;
            }
        } else {
            for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>)
                this[bufferName] = meshes[bufferName];

            const ranges = unwrapUndef(optionalRanges);
            for (const range of Object.keys(RANGE_TO_COUNT_TABLE) as Array<keyof PathRanges>)
                this[range] = ranges[range];
        }

        this.bQuadVertexPositionCount = this.bQuadVertexPositions.byteLength /
            B_QUAD_VERTEX_POSITION_SIZE;
        this.bQuadVertexInteriorIndexCount = this.bQuadVertexInteriorIndices.byteLength /
            INDEX_SIZE;
        this.bBoxCount = this.bBoxes.byteLength / (FLOAT32_SIZE * 6);
        this.stencilSegmentCount = this.stencilSegments.byteLength / (FLOAT32_SIZE * 6);

        this.rebuildPathIDBuffers();
    }

    expand(pathIDs: number[]): PathfinderMeshData {
        const tempOriginalBuffers: any = {}, tempExpandedArrays: any = {};
        for (const key of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>) {
            const arrayConstructor = PRIMITIVE_TYPE_ARRAY_CONSTRUCTORS[MESH_TYPES[key].type];
            tempOriginalBuffers[key] = new arrayConstructor(this[key]);
            tempExpandedArrays[key] = [];
        }

        const tempOriginalRanges: Partial<PathRanges> = {};
        const tempExpandedRanges: Partial<PathRanges> = {};
        for (const key of Object.keys(RANGE_TO_COUNT_TABLE) as Array<keyof PathRanges>) {
            tempOriginalRanges[key] = this[key];

            const newExpandedRanges = [];
            for (const pathIndex of pathIDs)
                newExpandedRanges.push(new Range(0, 0));
            tempExpandedRanges[key] = newExpandedRanges;
        }

        const originalBuffers: Meshes<PrimitiveTypeArray> = tempOriginalBuffers;
        const originalRanges: PathRanges = tempOriginalRanges as PathRanges;
        const expandedArrays: Meshes<number[]> = tempExpandedArrays;
        const expandedRanges: PathRanges = tempExpandedRanges as PathRanges;

        for (let newPathIndex = 0; newPathIndex < pathIDs.length; newPathIndex++) {
            const expandedPathID = newPathIndex + 1;
            const originalPathID = pathIDs[newPathIndex];

            // Copy over B-quad vertex positions.
            const bQuadVertexCopyResult = copyVertices(['bQuadVertexPositions'],
                                                       'bQuadVertexPositionPathRanges',
                                                       expandedArrays,
                                                       expandedRanges,
                                                       originalBuffers,
                                                       originalRanges,
                                                       expandedPathID,
                                                       originalPathID);

            if (bQuadVertexCopyResult == null)
                continue;

            const firstExpandedBQuadVertexIndex = bQuadVertexCopyResult.expandedStartIndex;
            const firstBQuadVertexIndex = bQuadVertexCopyResult.originalStartIndex;
            const lastBQuadVertexIndex = bQuadVertexCopyResult.originalEndIndex;

            // Copy over B-vertex indices.
            copyIndices(expandedArrays.bQuadVertexInteriorIndices,
                        expandedRanges.bQuadVertexInteriorIndexPathRanges,
                        originalBuffers.bQuadVertexInteriorIndices as Uint32Array,
                        firstExpandedBQuadVertexIndex * 6,
                        firstBQuadVertexIndex * 6,
                        lastBQuadVertexIndex * 6,
                        expandedPathID);

            // Copy over B-boxes.
            const bBoxVertexCopyResult = copyVertices(['bBoxes'],
                                                       'bBoxPathRanges',
                                                       expandedArrays,
                                                       expandedRanges,
                                                       originalBuffers,
                                                       originalRanges,
                                                       expandedPathID,
                                                       originalPathID);

            if (bBoxVertexCopyResult == null)
                continue;

            const firstExpandedBBoxIndex = bBoxVertexCopyResult.expandedStartIndex;
            const firstBBoxIndex = bBoxVertexCopyResult.originalStartIndex;
            const lastBBoxIndex = bBoxVertexCopyResult.originalEndIndex;

            // Copy over segments.
            copySegments(['stencilSegments', 'stencilNormals'],
                         'stencilSegmentPathRanges',
                         expandedArrays,
                         expandedRanges,
                         originalBuffers,
                         originalRanges,
                         expandedPathID,
                         originalPathID);
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
        return new PathfinderMeshData(expandedBuffers, expandedRanges);
    }

    private readPathRanges(meshes: ArrayBuffer): void {
        let offset = 0;
        while (offset < meshes.byteLength) {
            const fourCC = toFourCC(meshes, offset);
            const chunkLength = readUInt32(meshes, offset + 4);
            const startOffset = offset + 8;
            const endOffset = startOffset + chunkLength;

            if (PATH_RANGE_TYPE_FOURCCS.hasOwnProperty(fourCC)) {
                const key = PATH_RANGE_TYPE_FOURCCS[fourCC];
                const ranges = new Uint32Array(meshes.slice(startOffset, endOffset));
                this[key] = _.chunk(ranges, 2).map(range => new Range(range[0], range[1]));
            }

            offset = endOffset;
        }
    }

    private rebuildPathIDBuffers(): void {
        for (const rangeKey of Object.keys(RANGE_TO_COUNT_TABLE) as
             Array<keyof RangeToCountTable>) {
            if (!RANGE_TO_RANGE_BUFFER_TABLE.hasOwnProperty(rangeKey))
                continue;

            const rangeBufferKey = RANGE_TO_RANGE_BUFFER_TABLE[rangeKey];

            const instanceCount = this[RANGE_TO_COUNT_TABLE[rangeKey]];
            const ranges = this[rangeKey as keyof PathRanges];

            const meshType = MESH_TYPES[rangeBufferKey];
            const fieldCount = meshType.size;

            const destBuffer = new Uint16Array(instanceCount * fieldCount);
            let destIndex = 0;
            for (let pathIndex = 0; pathIndex < ranges.length; pathIndex++) {
                const range = ranges[pathIndex];
                for (let subindex = range.start; subindex < range.end; subindex++) {
                    for (let fieldIndex = 0; fieldIndex < fieldCount; fieldIndex++) {
                        destBuffer[destIndex] = pathIndex + 1;
                        destIndex++;
                    }
                }
            }

            (this as any)[rangeBufferKey] = destBuffer.buffer as ArrayBuffer;
        }
    }
}

export class PathfinderMeshBuffers implements Meshes<WebGLBuffer>, PathRanges {
    readonly bQuadVertexPositions: WebGLBuffer;
    readonly bQuadVertexPositionPathIDs: WebGLBuffer;
    readonly bQuadVertexInteriorIndices: WebGLBuffer;
    readonly bBoxes: WebGLBuffer;
    readonly bBoxSigns: WebGLBuffer;
    readonly bBoxIndices: WebGLBuffer;
    readonly bBoxPathIDs: WebGLBuffer;
    readonly stencilSegments: WebGLBuffer;
    readonly stencilSegmentPathIDs: WebGLBuffer;
    readonly stencilNormals: WebGLBuffer;

    readonly bQuadVertexPositionPathRanges: Range[];
    readonly bQuadVertexInteriorIndexPathRanges: Range[];
    readonly bBoxPathRanges: Range[];
    readonly stencilSegmentPathRanges: Range[];

    constructor(gl: WebGLRenderingContext, meshData: PathfinderMeshData) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>) {
            const bufferType = gl[BUFFER_TYPES[bufferName]];
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
            this[bufferName] = buffer;
        }

        for (const rangeName of RANGE_KEYS)
            this[rangeName] = meshData[rangeName];
    }
}

function copyVertices(vertexBufferNames: Array<keyof Meshes<void>>,
                      rangesName: keyof PathRanges,
                      expandedMeshes: Meshes<number[]>,
                      expandedRanges: PathRanges,
                      originalMeshes: Meshes<PrimitiveTypeArray>,
                      originalRanges: PathRanges,
                      expandedPathID: number,
                      originalPathID: number):
                      VertexCopyResult | null {
    const originalRange = originalRanges[rangesName][originalPathID - 1];

    const firstExpandedVertexIndex = _.reduce(expandedRanges[rangesName],
                                              (maxIndex, range) => Math.max(maxIndex, range.end),
                                              0);

    for (let originalVertexIndex = originalRange.start;
         originalVertexIndex < originalRange.end;
         originalVertexIndex++) {
        for (const vertexBufferName of vertexBufferNames) {
            const expanded = expandedMeshes[vertexBufferName];
            const original = originalMeshes[vertexBufferName];
            const size = MESH_TYPES[vertexBufferName].size;
            for (let elementIndex = 0; elementIndex < size; elementIndex++) {
                const globalIndex = size * originalVertexIndex + elementIndex;
                expanded.push(original[globalIndex]);
            }
        }
    }

    const lastExpandedVertexIndex = firstExpandedVertexIndex + originalRange.length;

    expandedRanges[rangesName][expandedPathID - 1] = new Range(firstExpandedVertexIndex,
                                                               lastExpandedVertexIndex);

    return {
        expandedEndIndex: lastExpandedVertexIndex,
        expandedStartIndex: firstExpandedVertexIndex,
        originalEndIndex: originalRange.end,
        originalStartIndex: originalRange.start,
    };
}

function copyIndices(destIndices: number[],
                     destRanges: Range[],
                     srcIndices: Uint32Array,
                     firstExpandedIndex: number,
                     firstIndex: number,
                     lastIndex: number,
                     expandedPathID: number,
                     validateIndex?: (indexIndex: number) => boolean) {
    if (firstIndex === lastIndex)
        return;

    // FIXME(pcwalton): Speed this up using the original ranges.
    let indexIndex = srcIndices.findIndex(index => index >= firstIndex && index < lastIndex);
    if (indexIndex < 0)
        return;

    const firstDestIndex = destIndices.length;
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

    const lastDestIndex = destIndices.length;

    destRanges[expandedPathID - 1] = new Range(firstDestIndex, lastDestIndex);
}

function copySegments(segmentBufferNames: Array<keyof Meshes<void>>,
                      rangesName: keyof PathRanges,
                      expandedMeshes: Meshes<number[]>,
                      expandedRanges: PathRanges,
                      originalMeshes: Meshes<PrimitiveTypeArray>,
                      originalRanges: PathRanges,
                      expandedPathID: number,
                      originalPathID: number):
                      void {
    const originalRange = originalRanges[rangesName][originalPathID - 1];

    const firstExpandedSegmentIndex = _.reduce(expandedRanges[rangesName],
                                               (maxIndex, range) => Math.max(maxIndex, range.end),
                                               0);

    for (let originalSegmentIndex = originalRange.start;
         originalSegmentIndex < originalRange.end;
         originalSegmentIndex++) {
        for (const segmentBufferName of segmentBufferNames) {
            if (originalMeshes[segmentBufferName].length === 0)
                continue;
            const size = MESH_TYPES[segmentBufferName].size;
            for (let fieldIndex = 0; fieldIndex < size; fieldIndex++) {
                const srcIndex = size * originalSegmentIndex + fieldIndex;
                expandedMeshes[segmentBufferName].push(originalMeshes[segmentBufferName][srcIndex]);
            }
        }
    }

    const lastExpandedSegmentIndex = firstExpandedSegmentIndex + originalRange.length;
    expandedRanges[rangesName][expandedPathID - 1] = new Range(firstExpandedSegmentIndex,
                                                               lastExpandedSegmentIndex);
}

function sizeOfPrimitive(primitiveType: PrimitiveType): number {
    switch (primitiveType) {
    case 'Uint16':  return UINT16_SIZE;
    case 'Uint32':  return UINT32_SIZE;
    case 'Float32': return FLOAT32_SIZE;
    }
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

function readUInt32(buffer: ArrayBuffer, offset: number): number {
    return (new Uint32Array(buffer.slice(offset, offset + 4)))[0];
}
