// pathfinder/client/src/mesh-debugger.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AppController} from "./app-controller";
import {OrthographicCamera} from "./camera";
import {B_QUAD_SIZE, B_QUAD_UPPER_LEFT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_UPPER_RIGHT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_UPPER_CONTROL_POINT_VERTEX_OFFSET, B_QUAD_LOWER_LEFT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_LOWER_RIGHT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_LOWER_CONTROL_POINT_VERTEX_OFFSET, PathfinderMeshData} from "./meshes";
import { BUILTIN_FONT_URI, GlyphStorage, PathfinderGlyph, TextRun } from "./text";
import { unwrapNull, UINT32_SIZE, UINT32_MAX, assert } from "./utils";
import {PathfinderView} from "./view";
import * as opentype from "opentype.js";

const CHARACTER: string = 'r';

const FONT: string = 'eb-garamond';

const POINT_LABEL_FONT: string = "12px sans-serif";
const POINT_LABEL_OFFSET: glmatrix.vec2 = glmatrix.vec2.fromValues(12.0, 12.0);
const POINT_RADIUS: number = 2.0;

class MeshDebuggerAppController extends AppController {
    start() {
        super.start();

        this.view = new MeshDebuggerView(this);

        this.loadInitialFile();
    }

    protected fileLoaded(): void {
        const font = opentype.parse(this.fileData);
        assert(font.isSupported(), "The font type is unsupported!");

        const createGlyph = (glyph: opentype.Glyph) => new MeshDebuggerGlyph(glyph);
        const textRun = new TextRun<MeshDebuggerGlyph>(CHARACTER, [0, 0], font, createGlyph);
        this.glyphStorage = new GlyphStorage(this.fileData, [textRun], createGlyph, font);

        this.glyphStorage.partition().then(meshes => {
            this.meshes = meshes;
            this.view.attachMeshes();
        })
    }

    protected get defaultFile(): string {
        return FONT;
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }

    glyphStorage: GlyphStorage<MeshDebuggerGlyph>;
    meshes: PathfinderMeshData;

    private view: MeshDebuggerView;
}

class MeshDebuggerView extends PathfinderView {
    constructor(appController: MeshDebuggerAppController) {
        super();

        this.appController = appController;
        this.camera = new OrthographicCamera(this.canvas);
        this.scale = 1.0;
    }

    attachMeshes() {
        this.setDirty();
    }

    redraw() {
        super.redraw();

        const meshes = this.appController.meshes;
        if (meshes == null)
            return;

        const context = unwrapNull(this.canvas.getContext('2d'));
        context.clearRect(0, 0, this.canvas.width, this.canvas.height);

        context.save();
        context.translate(this.camera.translation[0],
                          this.canvas.height - this.camera.translation[1]);
        context.scale(this.scale, this.scale);

        context.font = POINT_LABEL_FONT;

        const bQuads = new Uint32Array(meshes.bQuads);
        const positions = new Float32Array(meshes.bVertexPositions);

        const markedVertices: boolean[] = [];

        for (let bQuadIndex = 0; bQuadIndex < meshes.bQuadCount; bQuadIndex++) {
            const bQuadStartOffset = (B_QUAD_SIZE * bQuadIndex) / UINT32_SIZE;

            const upperLeftIndex = bQuads[bQuadStartOffset +
                                          B_QUAD_UPPER_LEFT_VERTEX_OFFSET / UINT32_SIZE];
            const upperRightIndex = bQuads[bQuadStartOffset +
                                           B_QUAD_UPPER_RIGHT_VERTEX_OFFSET / UINT32_SIZE];
            const upperControlPointIndex =
                bQuads[bQuadStartOffset + B_QUAD_UPPER_CONTROL_POINT_VERTEX_OFFSET / UINT32_SIZE];
            const lowerLeftIndex = bQuads[bQuadStartOffset +
                                          B_QUAD_LOWER_LEFT_VERTEX_OFFSET / UINT32_SIZE];
            const lowerRightIndex = bQuads[bQuadStartOffset +
                                           B_QUAD_LOWER_RIGHT_VERTEX_OFFSET / UINT32_SIZE];
            const lowerControlPointIndex =
                bQuads[bQuadStartOffset + B_QUAD_LOWER_CONTROL_POINT_VERTEX_OFFSET / UINT32_SIZE];

            const upperLeftPosition = unwrapNull(getPosition(positions, upperLeftIndex));
            const upperRightPosition = unwrapNull(getPosition(positions, upperRightIndex));
            const upperControlPointPosition = getPosition(positions, upperControlPointIndex);
            const lowerLeftPosition = unwrapNull(getPosition(positions, lowerLeftIndex));
            const lowerRightPosition = unwrapNull(getPosition(positions, lowerRightIndex));
            const lowerControlPointPosition = getPosition(positions, lowerControlPointIndex);

            drawVertexIfNecessary(context, markedVertices, upperLeftIndex, upperLeftPosition);
            drawVertexIfNecessary(context, markedVertices, upperRightIndex, upperRightPosition);
            drawVertexIfNecessary(context, markedVertices, lowerLeftIndex, lowerLeftPosition);
            drawVertexIfNecessary(context, markedVertices, lowerRightIndex, lowerRightPosition);

            context.beginPath();
            context.moveTo(upperLeftPosition[0], upperLeftPosition[1]);

            if (upperControlPointPosition != null) {
                context.quadraticCurveTo(upperControlPointPosition[0],
                                         upperControlPointPosition[1],
                                         upperRightPosition[0],
                                         upperRightPosition[1]);
            } else {
                context.lineTo(upperRightPosition[0], upperRightPosition[1]);
            }

            context.lineTo(lowerRightPosition[0], lowerRightPosition[1]);

            if (lowerControlPointPosition != null) {
                context.quadraticCurveTo(lowerControlPointPosition[0],
                                         lowerControlPointPosition[1],
                                         lowerLeftPosition[0],
                                         lowerLeftPosition[1]);
            } else {
                context.lineTo(lowerLeftPosition[0], lowerLeftPosition[1]);
            }

            context.closePath();
            context.stroke();
        }

        context.restore();
    }

    protected scale: number;

    private appController: MeshDebuggerAppController;

    camera: OrthographicCamera;
}

class MeshDebuggerGlyph extends PathfinderGlyph {}

function getPosition(positions: Float32Array, vertexIndex: number): Float32Array | null {
    if (vertexIndex == UINT32_MAX)
        return null;
    return new Float32Array([positions[vertexIndex * 2 + 0], -positions[vertexIndex * 2 + 1]]);
}

function drawVertexIfNecessary(context: CanvasRenderingContext2D,
                               markedVertices: boolean[],
                               vertexIndex: number,
                               position: Float32Array) {
    if (markedVertices[vertexIndex] != null)
        return;
    markedVertices[vertexIndex] = true;

    context.beginPath();
    context.moveTo(position[0], position[1]);
    context.arc(position[0], position[1], POINT_RADIUS, 0, 2.0 * Math.PI);
    context.fill();

    context.fillText("" + vertexIndex,
                     position[0] + POINT_LABEL_OFFSET[0],
                     position[1] + POINT_LABEL_OFFSET[1]);
}

function main() {
    const controller = new MeshDebuggerAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
