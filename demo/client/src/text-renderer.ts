// pathfinder/client/src/text-renderer.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import {Atlas, ATLAS_SIZE, AtlasGlyph, GlyphKey, SUBPIXEL_GRANULARITY} from './atlas';
import {CameraView, OrthographicCamera} from './camera';
import {createFramebuffer, createFramebufferDepthTexture, QUAD_ELEMENTS} from './gl-utils';
import {UniformMap} from './gl-utils';
import {Renderer} from './renderer';
import {ShaderMap} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {calculatePixelRectForGlyph, computeStemDarkeningAmount, GlyphStore, Hint} from "./text";
import {PathfinderFont, SimpleTextLayout, UnitMetrics} from "./text";
import {unwrapNull} from './utils';
import {RenderContext, Timings} from "./view";
import {AdaptiveMonochromeXCAAStrategy} from './xcaa-strategy';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveMonochromeXCAAStrategy;
}

const MIN_SCALE: number = 0.0025;
const MAX_SCALE: number = 0.5;

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: AdaptiveMonochromeXCAAStrategy,
};

export interface TextRenderContext extends RenderContext {
    atlasGlyphs: AtlasGlyph[];

    readonly cameraView: CameraView;
    readonly atlas: Atlas;
    readonly glyphStore: GlyphStore;
    readonly font: PathfinderFont;
    readonly fontSize: number;
    readonly useHinting: boolean;

    newTimingsReceived(timings: Timings): void;
}

export abstract class TextRenderer extends Renderer {
    renderContext: TextRenderContext;

    camera: OrthographicCamera;

    atlasFramebuffer: WebGLFramebuffer;
    atlasDepthTexture: WebGLTexture;

    get destFramebuffer(): WebGLFramebuffer {
        return this.atlasFramebuffer;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return ATLAS_SIZE;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.renderContext.atlas.usedSize;
    }

    get emboldenAmount(): glmatrix.vec2 {
        return this.stemDarkeningAmount;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    protected get layoutPixelsPerUnit(): number {
        return this.renderContext.fontSize / this.renderContext.font.opentypeFont.unitsPerEm;
    }

    protected get displayPixelsPerUnit(): number {
        return this.layoutPixelsPerUnit;
    }

    protected get worldTransform(): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / ATLAS_SIZE[0], 2.0 / ATLAS_SIZE[1], 1.0]);
        return transform;
    }

    protected get stemDarkeningAmount(): glmatrix.vec2 {
        if (this.stemDarkening === 'dark') {
            return computeStemDarkeningAmount(this.renderContext.fontSize,
                                              this.layoutPixelsPerUnit);
        }
        return glmatrix.vec2.create();
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.div(usedSize, this.renderContext.atlas.usedSize, ATLAS_SIZE);
        return usedSize;
    }

    private get pathCount(): number {
        return this.renderContext.glyphStore.glyphIDs.length * SUBPIXEL_GRANULARITY;
    }

    protected get objectCount(): number {
        return this.meshes.length;
    }

    private stemDarkening: StemDarkeningMode;
    private subpixelAA: SubpixelAAType;

    constructor(renderContext: TextRenderContext) {
        super(renderContext);

        this.camera = new OrthographicCamera(this.renderContext.cameraView, {
            maxScale: MAX_SCALE,
            minScale: MIN_SCALE,
        });
    }

    setHintsUniform(uniforms: UniformMap): void {
        const hint = this.createHint();
        this.renderContext.gl.uniform4f(uniforms.uHints,
                                        hint.xHeight,
                                        hint.hintedXHeight,
                                        hint.stemHeight,
                                        hint.hintedStemHeight);
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const pathCount = this.pathCount;
        const atlasGlyphs = this.renderContext.atlasGlyphs;
        const pixelsPerUnit = this.displayPixelsPerUnit;
        const font = this.renderContext.font;
        const hint = this.createHint();

        const boundingRects = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const atlasGlyphMetrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (atlasGlyphMetrics == null)
                continue;
            const atlasUnitMetrics = new UnitMetrics(atlasGlyphMetrics, this.stemDarkeningAmount);

            const pathID = glyph.pathID;
            boundingRects[pathID * 4 + 0] = atlasUnitMetrics.left;
            boundingRects[pathID * 4 + 1] = atlasUnitMetrics.descent;
            boundingRects[pathID * 4 + 2] = atlasUnitMetrics.right;
            boundingRects[pathID * 4 + 3] = atlasUnitMetrics.ascent;
        }

        return boundingRects;
    }

    protected createAtlasFramebuffer(): void {
        const atlasColorTexture = this.renderContext.atlas.ensureTexture(this.renderContext);
        this.atlasDepthTexture = createFramebufferDepthTexture(this.renderContext.gl, ATLAS_SIZE);
        this.atlasFramebuffer = createFramebuffer(this.renderContext.gl,
                                                  atlasColorTexture,
                                                  this.atlasDepthTexture);

        // Allow the antialiasing strategy to set up framebuffers as necessary.
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.setFramebufferSize(this);
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType,
                               stemDarkening: StemDarkeningMode):
                               AntialiasingStrategy {
        this.subpixelAA = subpixelAA;
        this.stemDarkening = stemDarkening;
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected clearForDirectRendering(): void {
        const gl = this.renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 1.0);
        gl.clearDepth(0.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected buildAtlasGlyphs(atlasGlyphs: AtlasGlyph[]): void {
        const font = this.renderContext.font;
        const displayPixelsPerUnit = this.displayPixelsPerUnit;
        const hint = this.createHint();

        atlasGlyphs.sort((a, b) => a.glyphKey.sortKey - b.glyphKey.sortKey);
        atlasGlyphs = _.sortedUniqBy(atlasGlyphs, glyph => glyph.glyphKey.sortKey);
        if (atlasGlyphs.length === 0)
            return;

        this.renderContext.atlasGlyphs = atlasGlyphs;
        this.renderContext.atlas.layoutGlyphs(atlasGlyphs,
                                              font,
                                              displayPixelsPerUnit,
                                              hint,
                                              this.stemDarkeningAmount);

        this.uploadPathTransforms(1);
        this.uploadPathColors(1);
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathCount = this.pathCount;

        const pathColors = new Uint8Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[(pathIndex + 1) * 4 + channel] = 0xff; // RGB
            pathColors[(pathIndex + 1) * 4 + 3] = 0xff;           // alpha
        }

        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const pathCount = this.pathCount;
        const atlasGlyphs = this.renderContext.atlasGlyphs;
        const pixelsPerUnit = this.displayPixelsPerUnit;

        // FIXME(pcwalton): This is a hack that tries to align glyphs on their baselines after
        // stem darkening. It's better than nothing, but we should really do better.
        const stemDarkeningOffset = glmatrix.vec2.clone(this.stemDarkeningAmount);
        glmatrix.vec2.scale(stemDarkeningOffset, stemDarkeningOffset, pixelsPerUnit);
        glmatrix.vec2.scale(stemDarkeningOffset, stemDarkeningOffset, 1.0 / Math.sqrt(2.0));

        const transforms = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const pathID = glyph.pathID;
            const atlasOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);

            transforms[pathID * 4 + 0] = pixelsPerUnit;
            transforms[pathID * 4 + 1] = pixelsPerUnit;
            transforms[pathID * 4 + 2] = atlasOrigin[0] + stemDarkeningOffset[0];
            transforms[pathID * 4 + 3] = atlasOrigin[1] + stemDarkeningOffset[1];
        }

        return transforms;
    }

    protected newTimingsReceived(): void {
        this.renderContext.newTimingsReceived(this.lastTimings);
    }

    protected createHint(): Hint {
        return new Hint(this.renderContext.font,
                        this.displayPixelsPerUnit,
                        this.renderContext.useHinting);
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}
