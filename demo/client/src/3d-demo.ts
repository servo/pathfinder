// pathfinder/client/src/3d-demo.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {DemoAppController} from "./app-controller";
import {mat4, vec2} from "gl-matrix";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import {BUILTIN_FONT_URI, TextLayout, PathfinderGlyph} from "./text";
import {PathfinderError, panic, unwrapNull} from "./utils";
import {PathfinderDemoView, Timings} from "./view";
import SSAAStrategy from "./ssaa-strategy";
import { OrthographicCamera } from "./camera";

const TEXT: string = "Lorem ipsum dolor sit amet";

const FONT: string = 'open-sans';

const PIXELS_PER_UNIT: number = 1.0;

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
};

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
}

class ThreeDController extends DemoAppController<ThreeDView> {
    start() {
        super.start();

        this.loadInitialFile();
    }

    protected fileLoaded(): void {
        this.layout = new TextLayout(this.fileData, TEXT, glyph => new ThreeDGlyph(glyph));
        this.layout.layoutText();
        this.layout.glyphStorage.partition().then((meshes: PathfinderMeshData) => {
            this.meshes = meshes;
            this.view.then(view => {
                view.uploadPathMetadata(this.layout.glyphStorage.textGlyphs.length);
                view.attachMeshes(this.meshes);
            });
        });
    }

    protected createView(): ThreeDView {
        return new ThreeDView(this,
                              unwrapNull(this.commonShaderSource),
                              unwrapNull(this.shaderSources));
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }

    protected get defaultFile(): string {
        return FONT;
    }

    layout: TextLayout<ThreeDGlyph>;
    private meshes: PathfinderMeshData;
}

class ThreeDView extends PathfinderDemoView {
    constructor(appController: ThreeDController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new OrthographicCamera(this.canvas);
        this.camera.onPan = () => this.setDirty();
        this.camera.onZoom = () => this.setDirty();
    }

    uploadPathMetadata(pathCount: number) {
        const textGlyphs = this.appController.layout.glyphStorage.textGlyphs;

        const pathColors = new Uint8Array(4 * (pathCount + 1));
        const pathTransforms = new Float32Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            const startOffset = (pathIndex + 1) * 4;

            for (let channel = 0; channel < 3; channel++)
                pathColors[startOffset + channel] = 0x00; // RGB
            pathColors[startOffset + 3] = 0xff;           // alpha

            const textGlyph = textGlyphs[pathIndex];
            const glyphRect = textGlyph.getRect(PIXELS_PER_UNIT);
            pathTransforms.set([1, 1, glyphRect[0], glyphRect[1]], startOffset);
        }

        this.pathColorsBufferTexture.upload(this.gl, pathColors);
        this.pathTransformBufferTexture.upload(this.gl, pathTransforms);
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName, aaLevel: number):
                               AntialiasingStrategy {
        if (aaType != 'ecaa')
            return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel);
        throw new PathfinderError("Unsupported antialiasing type!");
    }

    protected compositeIfNecessary(): void {}

    protected updateTimings(timings: Timings) {
        // TODO(pcwalton)
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width, this.canvas.height);
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(1.0, 1.0);
    }

    protected get worldTransform() {
        const transform = glmatrix.mat4.create();
        const translation = this.camera.translation;
        glmatrix.mat4.fromTranslation(transform, [translation[0], translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.camera.scale, this.camera.scale, 1.0]);
        return transform;
    }

    private _scale: number;

    private appController: ThreeDController;

    camera: OrthographicCamera;
}

class ThreeDGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        super(glyph);
    }

    getRect(pixelsPerUnit: number): glmatrix.vec4 {
        const rect =
            glmatrix.vec4.fromValues(this.position[0],
                                     this.position[1],
                                     this.position[0] + this.metrics.xMax - this.metrics.xMin,
                                     this.position[1] + this.metrics.yMax - this.metrics.yMin);
        glmatrix.vec4.scale(rect, rect, pixelsPerUnit);
        return rect;
    }
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
