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
import * as opentype from "opentype.js";

import {AppController} from "./app-controller";
import {OrthographicCamera} from "./camera";
import {FilePickerView} from './file-picker';
import {B_QUAD_SIZE, B_QUAD_UPPER_LEFT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_UPPER_RIGHT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_UPPER_CONTROL_POINT_VERTEX_OFFSET, B_QUAD_LOWER_LEFT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_LOWER_RIGHT_VERTEX_OFFSET} from "./meshes";
import {B_QUAD_LOWER_CONTROL_POINT_VERTEX_OFFSET, PathfinderMeshData} from "./meshes";
import {SVGLoader, BUILTIN_SVG_URI} from './svg-loader';
import {BUILTIN_FONT_URI, TextFrameGlyphStorage, PathfinderGlyph, TextRun} from "./text";
import {GlyphStorage, TextFrame} from "./text";
import {unwrapNull, UINT32_SIZE, UINT32_MAX, assert} from "./utils";
import {PathfinderView} from "./view";
import {Font} from 'opentype.js';

const CHARACTER: string = 'A';

const FONT: string = 'eb-garamond';

const POINT_LABEL_FONT: string = "sans-serif";
const POINT_LABEL_FONT_SIZE: number = 12.0;
const POINT_LABEL_OFFSET: glmatrix.vec2 = glmatrix.vec2.fromValues(12.0, 12.0);
const POINT_RADIUS: number = 2.0;

const LIGHT_STROKE_STYLE: string = "rgb(192, 192, 192)";
const LINE_STROKE_STYLE: string = "rgb(0, 128, 0)";
const CURVE_STROKE_STYLE: string = "rgb(128, 0, 0)";

const BUILTIN_URIS = {
    font: BUILTIN_FONT_URI,
    svg: BUILTIN_SVG_URI,
};

const SVG_SCALE: number = 1.0;

type FileType = 'font' | 'svg';

class MeshDebuggerAppController extends AppController {
    start() {
        super.start();

        this.fileType = 'font';

        this.view = new MeshDebuggerView(this);

        this.filePicker = unwrapNull(FilePickerView.create());
        this.filePicker.onFileLoaded = fileData => this.fileLoaded(fileData);

        this.openModal = unwrapNull(document.getElementById('pf-open-modal'));
        this.fontPathSelectGroup =
            unwrapNull(document.getElementById('pf-font-path-select-group'));
        this.fontPathSelect = unwrapNull(document.getElementById('pf-font-path-select')) as
            HTMLSelectElement;

        this.openFileSelect = unwrapNull(document.getElementById('pf-open-file-select')) as
            HTMLSelectElement;
        this.openFileSelect.addEventListener('click', () => this.openSelectedFile(), false);

        const openButton = unwrapNull(document.getElementById('pf-open-button'));
        openButton.addEventListener('click', () => this.showOpenDialog(), false);

        const openOKButton = unwrapNull(document.getElementById('pf-open-ok-button'));
        openOKButton.addEventListener('click', () => this.loadPath(), false);

        this.loadInitialFile(BUILTIN_FONT_URI);
    }

    private showOpenDialog(): void {
        window.jQuery(this.openModal).modal();
    }

    private openSelectedFile(): void {
        const selectedOption = this.openFileSelect.selectedOptions[0] as HTMLOptionElement;
        const optionValue = selectedOption.value;

        this.fontPathSelectGroup.classList.add('pf-display-none');

        const results = unwrapNull(/^([a-z]+)-(.*)$/.exec(optionValue));
        this.fileType = results[1] as FileType;

        const filename = results[2];
        if (filename === 'custom')
            this.filePicker.open();
        else
            this.fetchFile(results[2], BUILTIN_URIS[this.fileType]);
    }

    protected fileLoaded(fileData: ArrayBuffer): void {
        while (this.fontPathSelect.lastChild != null)
            this.fontPathSelect.removeChild(this.fontPathSelect.lastChild);

        this.fontPathSelectGroup.classList.remove('pf-display-none');

        if (this.fileType === 'font')
            this.fontLoaded(fileData);
        else if (this.fileType === 'svg')
            this.svgLoaded(fileData);
    }

    private fontLoaded(fileData: ArrayBuffer): void {
        this.file = opentype.parse(fileData);
        assert(this.file.isSupported(), "The font type is unsupported!");
        this.fileData = fileData;

        const glyphCount = this.file.numGlyphs;
        for (let glyphIndex = 1; glyphIndex < glyphCount; glyphIndex++) {
            const newOption = document.createElement('option');
            newOption.value = "" + glyphIndex;
            const glyphName = this.file.glyphIndexToName(glyphIndex);
            newOption.appendChild(document.createTextNode(glyphName));
            this.fontPathSelect.appendChild(newOption);
        }

        // Automatically load a path if this is the initial pageload.
        if (this.meshes == null)
            this.loadPath(this.file.charToGlyph(CHARACTER));
    }

    private svgLoaded(fileData: ArrayBuffer): void {
        this.file = new SVGLoader;
        this.file.scale = SVG_SCALE;
        this.file.loadFile(fileData);

        const pathCount = this.file.pathInstances.length;
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            const newOption = document.createElement('option');
            newOption.value = "" + pathIndex;
            newOption.appendChild(document.createTextNode(`Path ${pathIndex}`));
            this.fontPathSelect.appendChild(newOption);
        }
    }

    protected loadPath(opentypeGlyph?: opentype.Glyph | null) {
        window.jQuery(this.openModal).modal('hide');

        let promise: Promise<PathfinderMeshData>;

        if (this.file instanceof opentype.Font && this.fileData != null) {
            if (opentypeGlyph == null) {
                const glyphIndex = parseInt(this.fontPathSelect.selectedOptions[0].value);
                opentypeGlyph = this.file.glyphs.get(glyphIndex);
            }

            const glyph = new MeshDebuggerGlyph(opentypeGlyph);
            const glyphStorage = new GlyphStorage(this.fileData, [glyph], this.file);
            promise = glyphStorage.partition();
        } else if (this.file instanceof SVGLoader) {
            promise = this.file.partition(this.fontPathSelect.selectedIndex);
        } else {
            return;
        }

        promise.then(meshes => {
            this.meshes = meshes;
            this.view.attachMeshes();
        })
    }

    protected readonly defaultFile: string = FONT;

    private file: Font | SVGLoader | null;
    private fileType: FileType;
    private fileData: ArrayBuffer | null;

    meshes: PathfinderMeshData | null;

    private openModal: HTMLElement;
    private openFileSelect: HTMLSelectElement;
    private fontPathSelectGroup: HTMLElement;
    private fontPathSelect: HTMLSelectElement;

    private filePicker: FilePickerView;
    private view: MeshDebuggerView;
}

class MeshDebuggerView extends PathfinderView {
    constructor(appController: MeshDebuggerAppController) {
        super();

        this.appController = appController;
        this.camera = new OrthographicCamera(this.canvas, { ignoreBounds: true });

        this.camera.onPan = () => this.setDirty();
        this.camera.onZoom = () => this.setDirty();
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
        context.scale(this.camera.scale, this.camera.scale);

        const invScaleFactor = window.devicePixelRatio / this.camera.scale;
        context.font = `12px ${POINT_LABEL_FONT}`;
        context.lineWidth = invScaleFactor;

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

            drawVertexIfNecessary(context,
                                  markedVertices,
                                  upperLeftIndex,
                                  upperLeftPosition,
                                  invScaleFactor);
            drawVertexIfNecessary(context,
                                  markedVertices,
                                  upperRightIndex,
                                  upperRightPosition,
                                  invScaleFactor);
            drawVertexIfNecessary(context,
                                  markedVertices,
                                  lowerLeftIndex,
                                  lowerLeftPosition,
                                  invScaleFactor);
            drawVertexIfNecessary(context,
                                  markedVertices,
                                  lowerRightIndex,
                                  lowerRightPosition,
                                  invScaleFactor);

            context.beginPath();
            context.moveTo(upperLeftPosition[0], upperLeftPosition[1]);
            if (upperControlPointPosition != null) {
                context.strokeStyle = CURVE_STROKE_STYLE;
                context.quadraticCurveTo(upperControlPointPosition[0],
                                         upperControlPointPosition[1],
                                         upperRightPosition[0],
                                         upperRightPosition[1]);
            } else {
                context.strokeStyle = LINE_STROKE_STYLE;
                context.lineTo(upperRightPosition[0], upperRightPosition[1]);
            }
            context.stroke();

            context.strokeStyle = LIGHT_STROKE_STYLE;
            context.beginPath();
            context.moveTo(upperRightPosition[0], upperRightPosition[1]);
            context.lineTo(lowerRightPosition[0], lowerRightPosition[1]);
            context.stroke();

            context.beginPath();
            context.moveTo(lowerRightPosition[0], lowerRightPosition[1]);
            if (lowerControlPointPosition != null) {
                context.strokeStyle = CURVE_STROKE_STYLE;
                context.quadraticCurveTo(lowerControlPointPosition[0],
                                         lowerControlPointPosition[1],
                                         lowerLeftPosition[0],
                                         lowerLeftPosition[1]);
            } else {
                context.strokeStyle = LINE_STROKE_STYLE;
                context.lineTo(lowerLeftPosition[0], lowerLeftPosition[1]);
            }
            context.stroke();

            context.strokeStyle = LIGHT_STROKE_STYLE;
            context.beginPath();
            context.moveTo(lowerLeftPosition[0], lowerLeftPosition[1]);
            context.lineTo(upperLeftPosition[0], upperLeftPosition[1]);
            context.stroke();
        }

        context.restore();
    }

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
                               position: Float32Array,
                               invScaleFactor: number) {
    if (markedVertices[vertexIndex] != null)
        return;
    markedVertices[vertexIndex] = true;

    context.beginPath();
    context.moveTo(position[0], position[1]);
    context.arc(position[0], position[1], POINT_RADIUS * invScaleFactor, 0, 2.0 * Math.PI);
    context.fill();

    context.save();
    context.scale(invScaleFactor, invScaleFactor);
    context.fillText("" + vertexIndex,
                     position[0] / invScaleFactor + POINT_LABEL_OFFSET[0],
                     position[1] / invScaleFactor + POINT_LABEL_OFFSET[1]);
    context.restore();
}

function main() {
    const controller = new MeshDebuggerAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
