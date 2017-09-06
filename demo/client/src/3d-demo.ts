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
import {PerspectiveCamera} from "./camera";
import {mat4, vec2} from "gl-matrix";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import {BUILTIN_FONT_URI, TextLayout, PathfinderGlyph} from "./text";
import {PathfinderError, panic, unwrapNull} from "./utils";
import {PathfinderDemoView, Timings} from "./view";
import SSAAStrategy from "./ssaa-strategy";

const TEXT: string = "Lorem ipsum dolor sit amet";

const FONT: string = 'open-sans';

const PIXELS_PER_UNIT: number = 1.0;

const FOV: number = 45.0;
const NEAR_CLIP_PLANE: number = 0.01;
const FAR_CLIP_PLANE: number = 10000.0;

const SCALE: glmatrix.vec3 = glmatrix.vec3.fromValues(1.0 / 50.0, 1.0 / 50.0, 1.0);

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
        this.layout.glyphStorage.partition().then((baseMeshes: PathfinderMeshData) => {
            this.baseMeshes = baseMeshes;
            this.expandedMeshes = this.layout.glyphStorage.expandMeshes(baseMeshes).meshes;
            this.view.then(view => {
                view.uploadPathMetadata(this.layout.glyphStorage.textGlyphs.length);
                view.attachMeshes(this.expandedMeshes);
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

    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: PathfinderMeshData;
}

class ThreeDView extends PathfinderDemoView {
    constructor(appController: ThreeDController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new PerspectiveCamera(this.canvas);
        this.camera.onChange = () => this.setDirty();
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
            const glyphRect = textGlyph.pixelRect(PIXELS_PER_UNIT);
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
        const projection = glmatrix.mat4.create();
        glmatrix.mat4.perspective(projection,
                                  FOV / 180.0 * Math.PI,
                                  this.canvas.width / this.canvas.height,
                                  NEAR_CLIP_PLANE,
                                  FAR_CLIP_PLANE);

        const modelview = glmatrix.mat4.create();
        glmatrix.mat4.mul(modelview, modelview, this.camera.rotationMatrix);
        glmatrix.mat4.translate(modelview, modelview, this.camera.translation);
        glmatrix.mat4.scale(modelview, modelview, SCALE);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.mul(transform, projection, modelview);
        return transform;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'direct3DCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'direct3DInterior';
    }

    private _scale: number;

    private appController: ThreeDController;

    camera: PerspectiveCamera;
}

class ThreeDGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        super(glyph);
    }
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
