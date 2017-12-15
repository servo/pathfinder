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
import {expectNotNull, FLOAT32_SIZE, panic, PathfinderError, Range, UINT16_SIZE} from './utils';
import {UINT32_MAX, UINT32_SIZE, unwrapNull, unwrapUndef} from './utils';

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
    bQuadVertexPositions: { type: 'Float32', size: 12 },
    bQuads: { type: 'Uint32', size: B_QUAD_FIELD_COUNT },
    bVertexLoopBlinnData: { type: 'Uint32', size: 1 },
    bVertexNormals: { type: 'Float32', size: 1 },
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
    segmentCurveNormals: { type: 'Float32', size: 3 },
    segmentCurvePathIDs: { type: 'Uint16', size: 1 },
    segmentCurves: { type: 'Float32', size: 6 },
    segmentLineNormals: { type: 'Float32', size: 2 },
    segmentLinePathIDs: { type: 'Uint16', size: 1 },
    segmentLines: { type: 'Float32', size: 4 },
};

const BUFFER_TYPES: Meshes<BufferType> = {
    bQuadVertexPositions: 'ARRAY_BUFFER',
    bQuads: 'ARRAY_BUFFER',
    bVertexLoopBlinnData: 'ARRAY_BUFFER',
    bVertexNormals: 'ARRAY_BUFFER',
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
    segmentCurveNormals: 'ARRAY_BUFFER',
    segmentCurvePathIDs: 'ARRAY_BUFFER',
    segmentCurves: 'ARRAY_BUFFER',
    segmentLineNormals: 'ARRAY_BUFFER',
    segmentLinePathIDs: 'ARRAY_BUFFER',
    segmentLines: 'ARRAY_BUFFER',
};

const EDGE_BUFFER_NAMES = ['UpperLine', 'UpperCurve', 'LowerLine', 'LowerCurve'];

const RIFF_FOURCC: string = 'RIFF';

const MESH_LIBRARY_FOURCC: string = 'PFML';

// Must match the FourCCs in `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into()`.
const BUFFER_TYPE_FOURCCS: BufferTypeFourCCTable = {
    bqua: 'bQuads',
    bqvp: 'bQuadVertexPositions',
    bvlb: 'bVertexLoopBlinnData',
    bvno: 'bVertexNormals',
    bvpo: 'bVertexPositions',
    cvci: 'coverCurveIndices',
    cvii: 'coverInteriorIndices',
    ebbv: 'edgeBoundingBoxVertexPositions',
    elcv: 'edgeLowerCurveVertexPositions',
    ellv: 'edgeLowerLineVertexPositions',
    eucv: 'edgeUpperCurveVertexPositions',
    eulv: 'edgeUpperLineVertexPositions',
    scur: 'segmentCurves',
    slin: 'segmentLines',
    sncu: 'segmentCurveNormals',
    snli: 'segmentLineNormals',
};

// Must match the FourCCs in
// `pathfinder_partitioner::mesh_library::MeshLibrary::serialize_into::write_path_ranges()`.
const PATH_RANGE_TYPE_FOURCCS: PathRangeTypeFourCCTable = {
    bqua: 'bQuadPathRanges',
    bqvp: 'bQuadVertexPositionPathRanges',
    bver: 'bVertexPathRanges',
    cvci: 'coverCurveIndexRanges',
    cvii: 'coverInteriorIndexRanges',
    ebbo: 'edgeBoundingBoxRanges',
    elci: 'edgeLowerCurveIndexRanges',
    elli: 'edgeLowerLineIndexRanges',
    euci: 'edgeUpperCurveIndexRanges',
    euli: 'edgeUpperLineIndexRanges',
    scur: 'segmentCurveRanges',
    slin: 'segmentLineRanges',
};

const RANGE_TO_COUNT_TABLE: RangeToCountTable = {
    bQuadPathRanges: 'bQuadCount',
    bQuadVertexPositionPathRanges: 'bQuadVertexPositionCount',
    bVertexPathRanges: 'bVertexCount',
    coverCurveIndexRanges: 'coverCurveCount',
    coverInteriorIndexRanges: 'coverInteriorCount',
    edgeBoundingBoxRanges: 'edgeBoundingBoxCount',
    edgeLowerCurveIndexRanges: 'edgeLowerCurveCount',
    edgeLowerLineIndexRanges: 'edgeLowerLineCount',
    edgeUpperCurveIndexRanges: 'edgeUpperCurveCount',
    edgeUpperLineIndexRanges: 'edgeUpperLineCount',
    segmentCurveRanges: 'segmentCurveCount',
    segmentLineRanges: 'segmentLineCount',
};

const RANGE_TO_RANGE_BUFFER_TABLE: RangeToRangeBufferTable = {
    bVertexPathRanges: 'bVertexPathIDs',
    edgeBoundingBoxRanges: 'edgeBoundingBoxPathIDs',
    edgeLowerCurveIndexRanges: 'edgeLowerCurvePathIDs',
    edgeLowerLineIndexRanges: 'edgeLowerLinePathIDs',
    edgeUpperCurveIndexRanges: 'edgeUpperCurvePathIDs',
    edgeUpperLineIndexRanges: 'edgeUpperLinePathIDs',
    segmentCurveRanges: 'segmentCurvePathIDs',
    segmentLineRanges: 'segmentLinePathIDs',
};

const RANGE_KEYS: Array<keyof PathRanges> = [
    'bQuadPathRanges',
    'bQuadVertexPositionPathRanges',
    'bVertexPathRanges',
    'coverInteriorIndexRanges',
    'coverCurveIndexRanges',
    'edgeBoundingBoxRanges',
    'edgeUpperLineIndexRanges',
    'edgeUpperCurveIndexRanges',
    'edgeLowerLineIndexRanges',
    'edgeLowerCurveIndexRanges',
    'segmentCurveRanges',
    'segmentLineRanges',
];

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

export interface Meshes<T> {
    readonly bQuads: T;
    readonly bQuadVertexPositions: T;
    readonly bVertexPositions: T;
    readonly bVertexLoopBlinnData: T;
    readonly bVertexNormals: T;
    readonly coverInteriorIndices: T;
    readonly coverCurveIndices: T;
    readonly edgeBoundingBoxVertexPositions: T;
    readonly edgeLowerCurveVertexPositions: T;
    readonly edgeLowerLineVertexPositions: T;
    readonly edgeUpperCurveVertexPositions: T;
    readonly edgeUpperLineVertexPositions: T;
    readonly segmentLines: T;
    readonly segmentCurves: T;
    readonly segmentLineNormals: T;
    readonly segmentCurveNormals: T;

    bVertexPathIDs: T;
    edgeBoundingBoxPathIDs: T;
    segmentLinePathIDs: T;
    segmentCurvePathIDs: T;
    edgeLowerCurvePathIDs: T;
    edgeLowerLinePathIDs: T;
    edgeUpperCurvePathIDs: T;
    edgeUpperLinePathIDs: T;
}

interface MeshDataCounts {
    readonly bQuadCount: number;
    readonly bQuadVertexPositionCount: number;
    readonly bVertexCount: number;
    readonly coverCurveCount: number;
    readonly coverInteriorCount: number;
    readonly edgeBoundingBoxCount: number;
    readonly edgeLowerCurveCount: number;
    readonly edgeUpperCurveCount: number;
    readonly edgeLowerLineCount: number;
    readonly edgeUpperLineCount: number;
    readonly segmentLineCount: number;
    readonly segmentCurveCount: number;
}

interface PathRanges {
    bQuadPathRanges: Range[];
    bQuadVertexPositionPathRanges: Range[];
    bVertexPathRanges: Range[];
    coverInteriorIndexRanges: Range[];
    coverCurveIndexRanges: Range[];
    edgeBoundingBoxRanges: Range[];
    edgeUpperLineIndexRanges: Range[];
    edgeUpperCurveIndexRanges: Range[];
    edgeLowerLineIndexRanges: Range[];
    edgeLowerCurveIndexRanges: Range[];
    segmentCurveRanges: Range[];
    segmentLineRanges: Range[];
}

export class PathfinderMeshData implements Meshes<ArrayBuffer>, MeshDataCounts, PathRanges {
    readonly bQuads: ArrayBuffer;
    readonly bQuadVertexPositions: ArrayBuffer;
    readonly bVertexPositions: ArrayBuffer;
    readonly bVertexLoopBlinnData: ArrayBuffer;
    readonly bVertexNormals: ArrayBuffer;
    readonly coverInteriorIndices: ArrayBuffer;
    readonly coverCurveIndices: ArrayBuffer;
    readonly edgeBoundingBoxVertexPositions: ArrayBuffer;
    readonly edgeLowerCurveVertexPositions: ArrayBuffer;
    readonly edgeLowerLineVertexPositions: ArrayBuffer;
    readonly edgeUpperCurveVertexPositions: ArrayBuffer;
    readonly edgeUpperLineVertexPositions: ArrayBuffer;
    readonly segmentLines: ArrayBuffer;
    readonly segmentCurves: ArrayBuffer;
    readonly segmentLineNormals: ArrayBuffer;
    readonly segmentCurveNormals: ArrayBuffer;

    readonly bQuadCount: number;
    readonly bQuadVertexPositionCount: number;
    readonly bVertexCount: number;
    readonly coverCurveCount: number;
    readonly coverInteriorCount: number;
    readonly edgeBoundingBoxCount: number;
    readonly edgeLowerCurveCount: number;
    readonly edgeUpperCurveCount: number;
    readonly edgeLowerLineCount: number;
    readonly edgeUpperLineCount: number;
    readonly segmentLineCount: number;
    readonly segmentCurveCount: number;

    bVertexPathIDs: ArrayBuffer;
    edgeBoundingBoxPathIDs: ArrayBuffer;
    edgeLowerCurvePathIDs: ArrayBuffer;
    edgeLowerLinePathIDs: ArrayBuffer;
    edgeUpperCurvePathIDs: ArrayBuffer;
    edgeUpperLinePathIDs: ArrayBuffer;
    segmentCurvePathIDs: ArrayBuffer;
    segmentLinePathIDs: ArrayBuffer;

    bQuadPathRanges: Range[];
    bQuadVertexPositionPathRanges: Range[];
    bVertexPathRanges: Range[];
    coverInteriorIndexRanges: Range[];
    coverCurveIndexRanges: Range[];
    edgeBoundingBoxRanges: Range[];
    edgeUpperLineIndexRanges: Range[];
    edgeUpperCurveIndexRanges: Range[];
    edgeLowerLineIndexRanges: Range[];
    edgeLowerCurveIndexRanges: Range[];
    segmentCurveRanges: Range[];
    segmentLineRanges: Range[];

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

        this.bQuadCount = this.bQuads.byteLength / B_QUAD_SIZE;
        this.bQuadVertexPositionCount = this.bQuadVertexPositions.byteLength /
            B_QUAD_VERTEX_POSITION_SIZE;
        this.bVertexCount = this.bVertexPositions.byteLength / B_VERTEX_POSITION_SIZE;
        this.coverCurveCount = this.coverCurveIndices.byteLength / INDEX_SIZE;
        this.coverInteriorCount = this.coverInteriorIndices.byteLength / INDEX_SIZE;
        this.edgeBoundingBoxCount = this.edgeBoundingBoxVertexPositions.byteLength /
            EDGE_BOUNDING_BOX_VERTEX_POSITION_SIZE;
        this.edgeUpperLineCount = this.edgeUpperLineVertexPositions.byteLength /
            EDGE_UPPER_LINE_VERTEX_POSITION_SIZE;
        this.edgeLowerLineCount = this.edgeLowerLineVertexPositions.byteLength /
            EDGE_LOWER_LINE_VERTEX_POSITION_SIZE;
        this.edgeUpperCurveCount = this.edgeUpperCurveVertexPositions.byteLength /
            EDGE_UPPER_CURVE_VERTEX_POSITION_SIZE;
        this.edgeLowerCurveCount = this.edgeLowerCurveVertexPositions.byteLength /
            EDGE_LOWER_CURVE_VERTEX_POSITION_SIZE;
        this.segmentCurveCount = this.segmentCurves.byteLength / SEGMENT_CURVE_SIZE;
        this.segmentLineCount = this.segmentLines.byteLength / SEGMENT_LINE_SIZE;

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

            const bVertexCopyResult = copyVertices(['bVertexPositions',
                                                    'bVertexLoopBlinnData',
                                                    'bVertexNormals'],
                                                   'bVertexPathRanges',
                                                   expandedArrays,
                                                   expandedRanges,
                                                   originalBuffers,
                                                   originalRanges,
                                                   expandedPathID,
                                                   originalPathID);

            if (bVertexCopyResult == null)
                continue;

            const firstExpandedBVertexIndex = bVertexCopyResult.expandedStartIndex;
            const firstBVertexIndex = bVertexCopyResult.originalStartIndex;
            const lastBVertexIndex = bVertexCopyResult.originalEndIndex;

            // Copy over B-quad vertex positions.
            copyVertices(['bQuadVertexPositions'],
                         'bQuadVertexPositionPathRanges',
                         expandedArrays,
                         expandedRanges,
                         originalBuffers,
                         originalRanges,
                         expandedPathID,
                         originalPathID);

            // Copy over edge data.
            copyVertices(['edgeBoundingBoxVertexPositions'],
                         'edgeBoundingBoxRanges',
                         expandedArrays,
                         expandedRanges,
                         originalBuffers,
                         originalRanges,
                         expandedPathID,
                         originalPathID);
            for (const edgeBufferName of EDGE_BUFFER_NAMES) {
                copyVertices([`edge${edgeBufferName}VertexPositions` as keyof Meshes<void>],
                             `edge${edgeBufferName}IndexRanges` as keyof PathRanges,
                             expandedArrays,
                             expandedRanges,
                             originalBuffers,
                             originalRanges,
                             expandedPathID,
                             originalPathID);
            }

            // Copy over indices.
            copyIndices(expandedArrays.coverInteriorIndices,
                        expandedRanges.coverInteriorIndexRanges,
                        originalBuffers.coverInteriorIndices as Uint32Array,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        lastBVertexIndex,
                        expandedPathID);
            copyIndices(expandedArrays.coverCurveIndices,
                        expandedRanges.coverCurveIndexRanges,
                        originalBuffers.coverCurveIndices as Uint32Array,
                        firstExpandedBVertexIndex,
                        firstBVertexIndex,
                        lastBVertexIndex,
                        expandedPathID);

            // Copy over B-quads.
            const originalBQuadRange = originalRanges.bQuadPathRanges[originalPathID - 1];
            const firstExpandedBQuadIndex = expandedArrays.bQuads.length / B_QUAD_FIELD_COUNT;
            expandedRanges.bQuadPathRanges[expandedPathID - 1] =
                new Range(firstExpandedBQuadIndex,
                          firstExpandedBQuadIndex + originalBQuadRange.length);
            const indexDelta = firstExpandedBVertexIndex - firstBVertexIndex;
            for (let bQuadIndex = originalBQuadRange.start;
                 bQuadIndex < originalBQuadRange.end;
                 bQuadIndex++) {
                const bQuad = originalBuffers.bQuads[bQuadIndex];
                for (let indexIndex = 0; indexIndex < B_QUAD_FIELD_COUNT; indexIndex++) {
                    const srcIndex = originalBuffers.bQuads[bQuadIndex * B_QUAD_FIELD_COUNT +
                                                            indexIndex];
                    if (srcIndex === UINT32_MAX)
                        expandedArrays.bQuads.push(srcIndex);
                    else
                        expandedArrays.bQuads.push(srcIndex + indexDelta);
                }
            }

            // Copy over segments.
            copySegments(['segmentLines', 'segmentLineNormals'],
                         'segmentLineRanges',
                         expandedArrays,
                         expandedRanges,
                         originalBuffers,
                         originalRanges,
                         expandedPathID,
                         originalPathID);
            copySegments(['segmentCurves', 'segmentCurveNormals'],
                         'segmentCurveRanges',
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

            const count = this[RANGE_TO_COUNT_TABLE[rangeKey]];
            const ranges = this[rangeKey as keyof PathRanges];

            const destBuffer = new Uint16Array(count);
            let destIndex = 0;
            for (let pathIndex = 0; pathIndex < ranges.length; pathIndex++) {
                const range = ranges[pathIndex];
                for (let subindex = range.start; subindex < range.end; subindex++) {
                    destBuffer[destIndex] = pathIndex + 1;
                    destIndex++;
                }
            }

            (this as any)[RANGE_TO_RANGE_BUFFER_TABLE[rangeKey]] = destBuffer;
        }
    }
}

export class PathfinderMeshBuffers implements Meshes<WebGLBuffer>, PathRanges {
    readonly bQuads: WebGLBuffer;
    readonly bQuadVertexPositions: WebGLBuffer;
    readonly bVertexPositions: WebGLBuffer;
    readonly bVertexPathIDs: WebGLBuffer;
    readonly bVertexLoopBlinnData: WebGLBuffer;
    readonly bVertexNormals: WebGLBuffer;
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
    readonly segmentLines: WebGLBuffer;
    readonly segmentCurves: WebGLBuffer;
    readonly segmentLinePathIDs: WebGLBuffer;
    readonly segmentCurvePathIDs: WebGLBuffer;
    readonly segmentLineNormals: WebGLBuffer;
    readonly segmentCurveNormals: WebGLBuffer;

    readonly bQuadPathRanges: Range[];
    readonly bQuadVertexPositionPathRanges: Range[];
    readonly bVertexPathRanges: Range[];
    readonly coverInteriorIndexRanges: Range[];
    readonly coverCurveIndexRanges: Range[];
    readonly edgeBoundingBoxRanges: Range[];
    readonly edgeUpperLineIndexRanges: Range[];
    readonly edgeUpperCurveIndexRanges: Range[];
    readonly edgeLowerLineIndexRanges: Range[];
    readonly edgeLowerCurveIndexRanges: Range[];
    readonly segmentCurveRanges: Range[];
    readonly segmentLineRanges: Range[];

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
