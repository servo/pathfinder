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
    [fourCC: string]: keyof MeshLike<void>;
}

interface PathRangeTypeFourCCTable {
    [fourCC: string]: keyof PathRanges;
}

interface RangeToCountTable {
    [rangeKey: string]: keyof MeshDataCounts;
}

type PathIDBufferTable = Partial<MeshLike<PackedMeshBufferType>>;

interface ArrayLike {
    readonly length: number;
}

interface VertexCopyResult {
    originalStartIndex: number;
    originalEndIndex: number;
    expandedStartIndex: number;
    expandedEndIndex: number;
}

type PrimitiveType = 'Uint16' | 'Uint32' | 'Float32';

type PrimitiveTypeArray = Float32Array | Uint16Array | Uint32Array;

type MeshBufferType = keyof MeshLike<void>;

type PackedMeshBufferType = keyof PackedMeshLike<void>;

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

const MESH_TYPES: PackedMeshLike<MeshBufferTypeDescriptor> = {
    bBoxPathIDs: { type: 'Uint16', size: 1 },
    bBoxes: { type: 'Float32', size: 20 },
    bQuadVertexInteriorIndices: { type: 'Uint32', size: 1 },
    bQuadVertexPositionPathIDs: { type: 'Uint16', size: 1 },
    bQuadVertexPositions: { type: 'Float32', size: 2 },
    stencilNormals: { type: 'Float32', size: 6 },
    stencilSegmentPathIDs: { type: 'Uint16', size: 1 },
    stencilSegments: { type: 'Float32', size: 6 },
};

const BUFFER_TYPES: PackedMeshLike<BufferType> = {
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

const MESH_PACK_FOURCC: string = 'PFMP';

const MESH_FOURCC: string = 'mesh';

// Must match the FourCCs in `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into()`.
const BUFFER_TYPE_FOURCCS: BufferTypeFourCCTable = {
    bbox: 'bBoxes',
    bqii: 'bQuadVertexInteriorIndices',
    bqvp: 'bQuadVertexPositions',
    snor: 'stencilNormals',
    sseg: 'stencilSegments',
};

const RANGE_TO_COUNT_TABLE: RangeToCountTable = {
    bBoxPathRanges: 'bBoxCount',
    bQuadVertexInteriorIndexPathRanges: 'bQuadVertexInteriorIndexCount',
    bQuadVertexPositionPathRanges: 'bQuadVertexPositionCount',
    stencilSegmentPathRanges: 'stencilSegmentCount',
};

const INDEX_TYPE_DESCRIPTOR_TABLE: {[P in MeshBufferType]?: IndexTypeDescriptor} = {
    bQuadVertexInteriorIndices: {
        bufferType: 'bQuadVertexPositions',
    },
};

const PATH_ID_BUFFER_TABLE: PathIDBufferTable = {
    bBoxes: 'bBoxPathIDs',
    bQuadVertexPositions: 'bQuadVertexPositionPathIDs',
    stencilSegments: 'stencilSegmentPathIDs',
};

const PATH_RANGE_TO_BUFFER_TYPE_TABLE: {[P in keyof PathRanges]: MeshBufferType} = {
    bBoxPathRanges: 'bBoxes',
    bQuadVertexInteriorIndexPathRanges: 'bQuadVertexInteriorIndices',
    bQuadVertexPositionPathRanges: 'bQuadVertexPositions',
    stencilSegmentPathRanges: 'stencilSegments',
};

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

export interface MeshBuilder<T> {
    bQuadVertexPositions: T;
    bQuadVertexInteriorIndices: T;
    bBoxes: T;
    stencilSegments: T;
    stencilNormals: T;
}

export interface PackedMeshBuilder<T> extends MeshBuilder<T> {
    bBoxPathIDs: T;
    bQuadVertexPositionPathIDs: T;
    stencilSegmentPathIDs: T;
}

export type MeshLike<T> = {
    readonly [P in keyof MeshBuilder<void>]: T;
};

export type PackedMeshLike<T> = {
    readonly [P in keyof PackedMeshBuilder<void>]: T;
};

interface PathRanges {
    readonly bBoxPathRanges: Range[];
    readonly bQuadVertexInteriorIndexPathRanges: Range[];
    readonly bQuadVertexPositionPathRanges: Range[];
    readonly stencilSegmentPathRanges: Range[];
}

interface MeshDataCounts {
    readonly bQuadVertexPositionCount: number;
    readonly bQuadVertexInteriorIndexCount: number;
    readonly bBoxCount: number;
    readonly stencilSegmentCount: number;
}

interface IndexTypeDescriptor {
    bufferType: MeshBufferType;
}

export class PathfinderMeshPack {
    meshes: PathfinderMesh[];

    constructor(meshes: ArrayBuffer) {
        this.meshes = [];

        // RIFF encoded data.
        if (toFourCC(meshes, 0) !== RIFF_FOURCC)
            panic("Supplied array buffer is not a mesh library (no RIFF header)!");
        if (toFourCC(meshes, 8) !== MESH_PACK_FOURCC)
            panic("Supplied array buffer is not a mesh library (no PFMP header)!");

        let offset = 12;
        while (offset < meshes.byteLength) {
            const fourCC = toFourCC(meshes, offset);
            const chunkLength = readUInt32(meshes, offset + 4);
            const startOffset = offset + 8;
            const endOffset = startOffset + chunkLength;

            if (fourCC === MESH_FOURCC)
                this.meshes.push(new PathfinderMesh(meshes.slice(startOffset, endOffset)));

            offset = endOffset;
        }
    }
}

export class PathfinderMesh implements MeshLike<ArrayBuffer> {
    bQuadVertexPositions!: ArrayBuffer;
    bQuadVertexInteriorIndices!: ArrayBuffer;
    bBoxes!: ArrayBuffer;
    stencilSegments!: ArrayBuffer;
    stencilNormals!: ArrayBuffer;

    constructor(data: ArrayBuffer) {
        let offset = 0;
        while (offset < data.byteLength) {
            const fourCC = toFourCC(data, offset);
            const chunkLength = readUInt32(data, offset + 4);
            const startOffset = offset + 8;
            const endOffset = startOffset + chunkLength;

            if (BUFFER_TYPE_FOURCCS.hasOwnProperty(fourCC))
                this[BUFFER_TYPE_FOURCCS[fourCC]] = data.slice(startOffset, endOffset);

            offset = endOffset;
        }

        for (const type of Object.keys(BUFFER_TYPE_FOURCCS) as Array<keyof MeshLike<void>>) {
            if (this[type] == null)
                this[type] = new ArrayBuffer(0);
        }
    }
}

export class PathfinderPackedMeshes implements PackedMeshLike<PrimitiveTypeArray>, PathRanges {
    readonly bBoxes!: Float32Array;
    readonly bQuadVertexInteriorIndices!: Uint32Array;
    readonly bQuadVertexPositions!: Float32Array;
    readonly stencilSegments!: Float32Array;
    readonly stencilNormals!: Float32Array;

    readonly bBoxPathIDs!: Uint16Array;
    readonly bQuadVertexPositionPathIDs!: Uint16Array;
    readonly stencilSegmentPathIDs!: Uint16Array;

    readonly bBoxPathRanges!: Range[];
    readonly bQuadVertexInteriorIndexPathRanges!: Range[];
    readonly bQuadVertexPositionPathRanges!: Range[];
    readonly stencilSegmentPathRanges!: Range[];

    /// NB: Mesh indices are 1-indexed.
    constructor(meshPack: PathfinderMeshPack, meshIndices?: number[]) {
        if (meshIndices == null)
            meshIndices = meshPack.meshes.map((value, index) => index + 1);

        const meshData: PackedMeshBuilder<number[]> = {
            bBoxPathIDs: [],
            bBoxes: [],
            bQuadVertexInteriorIndices: [],
            bQuadVertexPositionPathIDs: [],
            bQuadVertexPositions: [],
            stencilNormals: [],
            stencilSegmentPathIDs: [],
            stencilSegments: [],
        };
        const pathRanges: PathRanges = {
            bBoxPathRanges: [],
            bQuadVertexInteriorIndexPathRanges: [],
            bQuadVertexPositionPathRanges: [],
            stencilSegmentPathRanges: [],
        };

        for (let destMeshIndex = 0; destMeshIndex < meshIndices.length; destMeshIndex++) {
            const srcMeshIndex = meshIndices[destMeshIndex];
            const mesh = meshPack.meshes[srcMeshIndex - 1];

            for (const pathRangeType of Object.keys(pathRanges) as Array<keyof PathRanges>) {
                const bufferType = PATH_RANGE_TO_BUFFER_TYPE_TABLE[pathRangeType];
                const startIndex = bufferCount(meshData, bufferType);
                pathRanges[pathRangeType].push(new Range(startIndex, startIndex));
            }

            for (const indexType of Object.keys(BUFFER_TYPES) as MeshBufferType[]) {
                if (BUFFER_TYPES[indexType] !== 'ELEMENT_ARRAY_BUFFER')
                    continue;
                const indexTypeDescriptor = unwrapUndef(INDEX_TYPE_DESCRIPTOR_TABLE[indexType]);
                const offset = bufferCount(meshData, indexTypeDescriptor.bufferType);
                for (const index of new Uint32Array(mesh[indexType]))
                    meshData[indexType].push(index + offset);
            }
            for (const bufferType of Object.keys(BUFFER_TYPES) as MeshBufferType[]) {
                if (BUFFER_TYPES[bufferType] !== 'ARRAY_BUFFER')
                    continue;
                meshData[bufferType].push(...new Float32Array(mesh[bufferType]));

                const pathIDBufferType = PATH_ID_BUFFER_TABLE[bufferType];
                if (pathIDBufferType != null) {
                    const length = bufferCount(meshData, bufferType);
                    while (meshData[pathIDBufferType].length < length)
                        meshData[pathIDBufferType].push(destMeshIndex + 1);
                }
            }

            for (const pathRangeType of Object.keys(PATH_RANGE_TO_BUFFER_TYPE_TABLE) as
                 Array<keyof PathRanges>) {
                const bufferType = PATH_RANGE_TO_BUFFER_TYPE_TABLE[pathRangeType];
                const endIndex = bufferCount(meshData, bufferType);
                unwrapUndef(_.last(pathRanges[pathRangeType])).end = endIndex;
            }
        }

        for (const bufferType of Object.keys(BUFFER_TYPES) as PackedMeshBufferType[]) {
            const arrayCtor = PRIMITIVE_TYPE_ARRAY_CONSTRUCTORS[MESH_TYPES[bufferType].type];
            this[bufferType] = (new arrayCtor(meshData[bufferType])) as any;
        }
        _.assign(this, pathRanges);
    }

    count(bufferType: MeshBufferType): number {
        return bufferCount(this, bufferType);
    }
}

export class PathfinderPackedMeshBuffers implements PackedMeshLike<WebGLBuffer>, PathRanges {
    readonly bBoxes!: WebGLBuffer;
    readonly bQuadVertexInteriorIndices!: WebGLBuffer;
    readonly bQuadVertexPositions!: WebGLBuffer;
    readonly stencilSegments!: WebGLBuffer;
    readonly stencilNormals!: WebGLBuffer;

    readonly bBoxPathIDs!: WebGLBuffer;
    readonly bQuadVertexPositionPathIDs!: WebGLBuffer;
    readonly stencilSegmentPathIDs!: WebGLBuffer;

    readonly bBoxPathRanges!: Range[];
    readonly bQuadVertexInteriorIndexPathRanges!: Range[];
    readonly bQuadVertexPositionPathRanges!: Range[];
    readonly stencilSegmentPathRanges!: Range[];

    constructor(gl: WebGLRenderingContext, packedMeshes: PathfinderPackedMeshes) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as PackedMeshBufferType[]) {
            const bufferType = gl[BUFFER_TYPES[bufferName]];
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, packedMeshes[bufferName], gl.STATIC_DRAW);
            this[bufferName] = buffer;
        }

        for (const rangeName of Object.keys(PATH_RANGE_TO_BUFFER_TYPE_TABLE) as
             Array<keyof PathRanges>) {
            this[rangeName] = packedMeshes[rangeName];
        }
    }
}

function bufferCount(mesh: MeshLike<ArrayLike>, bufferType: MeshBufferType): number {
    return mesh[bufferType].length / MESH_TYPES[bufferType].size;
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
