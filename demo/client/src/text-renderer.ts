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
    readonly layout: SimpleTextLayout;
    readonly glyphStore: GlyphStore;
    readonly font: PathfinderFont;
    readonly fontSize: number;
    readonly pathCount: number;
    readonly layoutPixelsPerUnit: number;
    readonly useHinting: boolean;

    newTimingsReceived(timings: Timings): void;
}

export class TextRenderer extends Renderer {
    renderContext: TextRenderContext;

    camera: OrthographicCamera;

    atlasFramebuffer: WebGLFramebuffer;
    atlasDepthTexture: WebGLTexture;

    glyphPositionsBuffer: WebGLBuffer;
    glyphTexCoordsBuffer: WebGLBuffer;
    glyphElementsBuffer: WebGLBuffer;

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
        return glmatrix.vec4.fromValues(1.0, 1.0, 1.0, 0.0);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.fromValues(0.0, 0.0, 0.0, 1.0);
    }

    protected get worldTransform(): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / ATLAS_SIZE[0], 2.0 / ATLAS_SIZE[1], 1.0]);
        return transform;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }

    protected get depthFunction(): number {
        return this.renderContext.gl.GREATER;
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.div(usedSize, this.renderContext.atlas.usedSize, ATLAS_SIZE);
        return usedSize;
    }

    private get stemDarkeningAmount(): glmatrix.vec2 {
        if (this.stemDarkening === 'dark') {
            return computeStemDarkeningAmount(this.renderContext.fontSize,
                                              this.renderContext.layoutPixelsPerUnit);
        }
        return glmatrix.vec2.create();
    }

    private glyphBounds: Float32Array;
    private stemDarkening: StemDarkeningMode;
    private subpixelAA: SubpixelAAType;

    private get displayPixelsPerUnit(): number {
        return this.renderContext.layoutPixelsPerUnit;
    }

    constructor(renderContext: TextRenderContext) {
        super(renderContext);

        this.camera = new OrthographicCamera(this.renderContext.cameraView, {
            maxScale: MAX_SCALE,
            minScale: MIN_SCALE,
        });
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           subpixelAA: SubpixelAAType,
                           stemDarkening: StemDarkeningMode) {
        super.setAntialiasingOptions(aaType, aaLevel, subpixelAA, stemDarkening);

        // Need to relayout because changing AA options can cause font dilation to change...
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    setHintsUniform(uniforms: UniformMap): void {
        const hint = this.createHint();
        this.renderContext.gl.uniform4f(uniforms.uHints,
                                        hint.xHeight,
                                        hint.hintedXHeight,
                                        hint.stemHeight,
                                        hint.hintedStemHeight);
    }

    prepareToAttachText(): void {
        if (this.atlasFramebuffer == null)
            this.createAtlasFramebuffer();

        this.layoutText();
    }

    finishAttachingText(): void {
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    relayoutText(): void {
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    updateHinting(): void {
        // Need to relayout the text because the pixel bounds of the glyphs can change from this...
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    viewPanned(): void {
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const pathCount = this.renderContext.pathCount;
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
        this.renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        this.renderContext.gl.clearDepth(0.0);
        this.renderContext.gl.depthMask(true);
        this.renderContext.gl.clear(this.renderContext.gl.COLOR_BUFFER_BIT |
                                    this.renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected compositeIfNecessary() {
        // Set up composite state.
        this.renderContext.gl.bindFramebuffer(this.renderContext.gl.FRAMEBUFFER, null);
        this.renderContext.gl.viewport(0,
                                       0,
                                       this.renderContext.cameraView.width,
                                       this.renderContext.cameraView.height);
        this.renderContext.gl.disable(this.renderContext.gl.DEPTH_TEST);
        this.renderContext.gl.disable(this.renderContext.gl.SCISSOR_TEST);
        this.renderContext.gl.blendEquation(this.renderContext.gl.FUNC_ADD);
        this.renderContext.gl.blendFuncSeparate(this.renderContext.gl.SRC_ALPHA,
                                                this.renderContext.gl.ONE_MINUS_SRC_ALPHA,
                                                this.renderContext.gl.ONE,
                                                this.renderContext.gl.ONE);
        this.renderContext.gl.enable(this.renderContext.gl.BLEND);

        // Clear.
        this.renderContext.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        this.renderContext.gl.clear(this.renderContext.gl.COLOR_BUFFER_BIT);

        // Set up the composite VAO.
        const blitProgram = this.renderContext.shaderPrograms.blit;
        const attributes = blitProgram.attributes;
        this.renderContext.gl.useProgram(blitProgram.program);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphPositionsBuffer);
        this.renderContext.gl.vertexAttribPointer(attributes.aPosition,
                                                  2,
                                                  this.renderContext.gl.FLOAT,
                                                  false,
                                                  0,
                                                  0);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphTexCoordsBuffer);
        this.renderContext.gl.vertexAttribPointer(attributes.aTexCoord,
                                                  2,
                                                  this.renderContext.gl.FLOAT,
                                                  false,
                                                  0,
                                                  0);
        this.renderContext.gl.enableVertexAttribArray(attributes.aPosition);
        this.renderContext.gl.enableVertexAttribArray(attributes.aTexCoord);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.glyphElementsBuffer);

        // Create the transform.
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [
            2.0 / this.renderContext.cameraView.width,
            2.0 / this.renderContext.cameraView.height,
            1.0,
        ]);
        glmatrix.mat4.translate(transform,
                                transform,
                                [this.camera.translation[0], this.camera.translation[1], 0.0]);

        // Blit.
        this.renderContext.gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        this.renderContext.gl.activeTexture(this.renderContext.gl.TEXTURE0);
        const destTexture = this.renderContext
                                .atlas
                                .ensureTexture(this.renderContext);
        this.renderContext.gl.bindTexture(this.renderContext.gl.TEXTURE_2D, destTexture);
        this.renderContext.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        const totalGlyphCount = this.renderContext.layout.textFrame.totalGlyphCount;
        this.renderContext.gl.drawElements(this.renderContext.gl.TRIANGLES,
                                           totalGlyphCount * 6,
                                           this.renderContext.gl.UNSIGNED_INT,
                                           0);
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathCount = this.renderContext.pathCount;

        const pathColors = new Uint8Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[(pathIndex + 1) * 4 + channel] = 0x00; // RGB
            pathColors[(pathIndex + 1) * 4 + 3] = 0xff;           // alpha
        }

        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const pathCount = this.renderContext.pathCount;
        const atlasGlyphs = this.renderContext.atlasGlyphs;
        const pixelsPerUnit = this.displayPixelsPerUnit;

        const transforms = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const pathID = glyph.pathID;
            const atlasOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);

            transforms[pathID * 4 + 0] = pixelsPerUnit;
            transforms[pathID * 4 + 1] = pixelsPerUnit;
            transforms[pathID * 4 + 2] = atlasOrigin[0];
            transforms[pathID * 4 + 3] = atlasOrigin[1];
        }

        return transforms;
    }

    protected newTimingsReceived() {
        this.renderContext.newTimingsReceived(this.lastTimings);
    }

    private createAtlasFramebuffer() {
        const atlasColorTexture = this.renderContext.atlas.ensureTexture(this.renderContext);
        this.atlasDepthTexture = createFramebufferDepthTexture(this.renderContext.gl, ATLAS_SIZE);
        this.atlasFramebuffer = createFramebuffer(this.renderContext.gl,
                                                  this.renderContext.drawBuffersExt,
                                                  [atlasColorTexture],
                                                  this.atlasDepthTexture);

        // Allow the antialiasing strategy to set up framebuffers as necessary.
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.setFramebufferSize(this);
    }

    private createHint(): Hint {
        return new Hint(this.renderContext.font,
                        this.displayPixelsPerUnit,
                        this.renderContext.useHinting);
    }

    private layoutText() {
        const layout = this.renderContext.layout;
        layout.layoutRuns();

        const textBounds = layout.textFrame.bounds;
        this.camera.bounds = textBounds;

        const totalGlyphCount = layout.textFrame.totalGlyphCount;
        const glyphPositions = new Float32Array(totalGlyphCount * 8);
        const glyphIndices = new Uint32Array(totalGlyphCount * 6);

        const hint = this.createHint();
        const displayPixelsPerUnit = this.displayPixelsPerUnit;
        const layoutPixelsPerUnit = this.renderContext.layoutPixelsPerUnit;

        let globalGlyphIndex = 0;
        for (const run of layout.textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const rect = run.pixelRectForGlyphAt(glyphIndex,
                                                     layoutPixelsPerUnit,
                                                     displayPixelsPerUnit,
                                                     hint,
                                                     this.stemDarkeningAmount,
                                                     SUBPIXEL_GRANULARITY);
                glyphPositions.set([
                    rect[0], rect[3],
                    rect[2], rect[3],
                    rect[0], rect[1],
                    rect[2], rect[1],
                ], globalGlyphIndex * 8);

                for (let glyphIndexIndex = 0;
                    glyphIndexIndex < QUAD_ELEMENTS.length;
                    glyphIndexIndex++) {
                    glyphIndices[glyphIndexIndex + globalGlyphIndex * 6] =
                        QUAD_ELEMENTS[glyphIndexIndex] + 4 * globalGlyphIndex;
                }
            }
        }

        this.glyphPositionsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphPositionsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ARRAY_BUFFER,
                                         glyphPositions,
                                         this.renderContext.gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.glyphElementsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         glyphIndices,
                                         this.renderContext.gl.STATIC_DRAW);
    }

    private buildAtlasGlyphs() {
        const font = this.renderContext.font;
        const glyphStore = this.renderContext.glyphStore;
        const layoutPixelsPerUnit = this.renderContext.layoutPixelsPerUnit;
        const displayPixelsPerUnit = this.displayPixelsPerUnit;

        const textFrame = this.renderContext.layout.textFrame;
        const hint = this.createHint();

        // Only build glyphs in view.
        const translation = this.camera.translation;
        const canvasRect = glmatrix.vec4.clone([
            -translation[0],
            -translation[1],
            -translation[0] + this.renderContext.cameraView.width,
            -translation[1] + this.renderContext.cameraView.height,
        ]);

        let atlasGlyphs = [];
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                const pixelRect = run.pixelRectForGlyphAt(glyphIndex,
                                                          layoutPixelsPerUnit,
                                                          displayPixelsPerUnit,
                                                          hint,
                                                          this.stemDarkeningAmount,
                                                          SUBPIXEL_GRANULARITY);
                if (!rectsIntersect(pixelRect, canvasRect))
                    continue;

                const glyphID = run.glyphIDs[glyphIndex];
                const glyphStoreIndex = glyphStore.indexOfGlyphWithID(glyphID);
                if (glyphStoreIndex == null)
                    continue;

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        layoutPixelsPerUnit,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY);
                const glyphKey = new GlyphKey(glyphID, subpixel);
                atlasGlyphs.push(new AtlasGlyph(glyphStoreIndex, glyphKey));
            }
        }

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

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.

        this.setGlyphTexCoords();
    }

    private setGlyphTexCoords() {
        const textFrame = this.renderContext.layout.textFrame;
        const font = this.renderContext.font;
        const atlasGlyphs = this.renderContext.atlasGlyphs;

        const hint = this.createHint();
        const layoutPixelsPerUnit = this.renderContext.layoutPixelsPerUnit;
        const displayPixelsPerUnit = this.displayPixelsPerUnit;

        const atlasGlyphKeys = atlasGlyphs.map(atlasGlyph => atlasGlyph.glyphKey.sortKey);

        this.glyphBounds = new Float32Array(textFrame.totalGlyphCount * 8);

        let globalGlyphIndex = 0;
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const textGlyphID = run.glyphIDs[glyphIndex];

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        layoutPixelsPerUnit,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY);

                const glyphKey = new GlyphKey(textGlyphID, subpixel);

                const atlasGlyphIndex = _.sortedIndexOf(atlasGlyphKeys, glyphKey.sortKey);
                if (atlasGlyphIndex < 0)
                    continue;

                // Set texture coordinates.
                const atlasGlyph = atlasGlyphs[atlasGlyphIndex];
                const atlasGlyphMetrics = font.metricsForGlyph(atlasGlyph.glyphKey.id);
                if (atlasGlyphMetrics == null)
                    continue;

                const atlasGlyphUnitMetrics = new UnitMetrics(atlasGlyphMetrics,
                                                              this.stemDarkeningAmount);

                const atlasGlyphPixelOrigin =
                    atlasGlyph.calculateSubpixelOrigin(displayPixelsPerUnit);
                const atlasGlyphRect = calculatePixelRectForGlyph(atlasGlyphUnitMetrics,
                                                                  atlasGlyphPixelOrigin,
                                                                  displayPixelsPerUnit,
                                                                  hint);
                const atlasGlyphBL = atlasGlyphRect.slice(0, 2) as glmatrix.vec2;
                const atlasGlyphTR = atlasGlyphRect.slice(2, 4) as glmatrix.vec2;
                glmatrix.vec2.div(atlasGlyphBL, atlasGlyphBL, ATLAS_SIZE);
                glmatrix.vec2.div(atlasGlyphTR, atlasGlyphTR, ATLAS_SIZE);

                this.glyphBounds.set([
                    atlasGlyphBL[0], atlasGlyphTR[1],
                    atlasGlyphTR[0], atlasGlyphTR[1],
                    atlasGlyphBL[0], atlasGlyphBL[1],
                    atlasGlyphTR[0], atlasGlyphBL[1],
                ], globalGlyphIndex * 8);
            }
        }

        this.glyphTexCoordsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphTexCoordsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphBounds,
                                         this.renderContext.gl.STATIC_DRAW);
    }

    private setIdentityTexScaleUniform(uniforms: UniformMap) {
        this.renderContext.gl.uniform2f(uniforms.uTexScale, 1.0, 1.0);
    }
}

/// The separating axis theorem.
function rectsIntersect(a: glmatrix.vec4, b: glmatrix.vec4): boolean {
    return a[2] > b[0] && a[3] > b[1] && a[0] < b[2] && a[1] < b[3];
}
