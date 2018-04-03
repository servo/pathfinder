// pathfinder/client/src/text-renderer.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
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
import {AAOptions} from './app-controller';
import {Atlas, ATLAS_SIZE, AtlasGlyph, GlyphKey, SUBPIXEL_GRANULARITY} from './atlas';
import {CameraView, OrthographicCamera} from './camera';
import {createFramebuffer, createFramebufferDepthTexture, QUAD_ELEMENTS} from './gl-utils';
import {UniformMap} from './gl-utils';
import {PathTransformBuffers, Renderer} from './renderer';
import {ShaderMap} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {calculatePixelRectForGlyph, computeStemDarkeningAmount, GlyphStore, Hint} from "./text";
import {MAX_STEM_DARKENING_PIXELS_PER_EM, PathfinderFont, SimpleTextLayout} from "./text";
import {UnitMetrics} from "./text";
import {unwrapNull} from './utils';
import {RenderContext, Timings} from "./view";
import {StencilAAAStrategy} from './xcaa-strategy';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof StencilAAAStrategy;
}

const SQRT_1_2: number = 1.0 / Math.sqrt(2.0);

const MIN_SCALE: number = 0.0025;
const MAX_SCALE: number = 0.5;

export const MAX_SUBPIXEL_AA_FONT_SIZE: number = 48.0;

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: StencilAAAStrategy,
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
    renderContext!: TextRenderContext;

    camera: OrthographicCamera;

    atlasFramebuffer!: WebGLFramebuffer;
    atlasDepthTexture!: WebGLTexture;

    get isMulticolor(): boolean {
        return false;
    }

    get needsStencil(): boolean {
        return this.renderContext.fontSize <= MAX_STEM_DARKENING_PIXELS_PER_EM;
    }

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
        const emboldenAmount = glmatrix.vec2.create();
        glmatrix.vec2.add(emboldenAmount, this.extraEmboldenAmount, this.stemDarkeningAmount);
        return emboldenAmount;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    get rotationAngle(): number {
        return 0.0;
    }

    get allowSubpixelAA(): boolean {
        return this.renderContext.fontSize <= MAX_SUBPIXEL_AA_FONT_SIZE;
    }

    protected get pixelsPerUnit(): number {
        return this.renderContext.fontSize / this.renderContext.font.opentypeFont.unitsPerEm;
    }

    protected get worldTransform(): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / ATLAS_SIZE[0], 2.0 / ATLAS_SIZE[1], 1.0]);
        return transform;
    }

    protected get stemDarkeningAmount(): glmatrix.vec2 {
        if (this.stemDarkening === 'dark')
            return computeStemDarkeningAmount(this.renderContext.fontSize, this.pixelsPerUnit);
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
        return this.meshBuffers == null ? 0 : this.meshBuffers.length;
    }

    protected get extraEmboldenAmount(): glmatrix.vec2 {
        return glmatrix.vec2.create();
    }

    private stemDarkening!: StemDarkeningMode;
    private subpixelAA!: SubpixelAAType;

    constructor(renderContext: TextRenderContext) {
        super(renderContext);

        this.camera = new OrthographicCamera(this.renderContext.cameraView, {
            maxScale: MAX_SCALE,
            minScale: MIN_SCALE,
        });
    }

    setHintsUniform(uniforms: UniformMap): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const hint = this.createHint();
        gl.uniform4f(uniforms.uHints,
                     hint.xHeight,
                     hint.hintedXHeight,
                     hint.stemHeight,
                     hint.hintedStemHeight);
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const pathCount = this.pathCount;
        const atlasGlyphs = this.renderContext.atlasGlyphs;
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;
        const font = this.renderContext.font;
        const hint = this.createHint();

        const boundingRects = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const atlasGlyphMetrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (atlasGlyphMetrics == null)
                continue;
            const atlasUnitMetrics = new UnitMetrics(atlasGlyphMetrics, 0.0, this.emboldenAmount);

            const pathID = glyph.pathID;
            boundingRects[pathID * 4 + 0] = atlasUnitMetrics.left;
            boundingRects[pathID * 4 + 1] = atlasUnitMetrics.descent;
            boundingRects[pathID * 4 + 2] = atlasUnitMetrics.right;
            boundingRects[pathID * 4 + 3] = atlasUnitMetrics.ascent;
        }

        return boundingRects;
    }

    pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array> {
        const pathCount = this.pathCount;
        const atlasGlyphs = this.renderContext.atlasGlyphs;
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;

        // FIXME(pcwalton): This is a hack that tries to preserve the vertical extents of the glyph
        // after stem darkening. It's better than nothing, but we should really do better.
        //
        // This hack seems to produce *better* results than what macOS does on sans-serif fonts;
        // the ascenders and x-heights of the glyphs are pixel snapped, while they aren't on macOS.
        // But we should really figure out what macOS does…
        const ascender = this.renderContext.font.opentypeFont.ascender;
        const emboldenAmount = this.emboldenAmount;
        const stemDarkeningYScale = (ascender + emboldenAmount[1]) / ascender;

        const stemDarkeningOffset = glmatrix.vec2.clone(emboldenAmount);
        glmatrix.vec2.scale(stemDarkeningOffset, stemDarkeningOffset, pixelsPerUnit);
        glmatrix.vec2.scale(stemDarkeningOffset, stemDarkeningOffset, SQRT_1_2);
        glmatrix.vec2.mul(stemDarkeningOffset, stemDarkeningOffset, [1, stemDarkeningYScale]);

        const transform = glmatrix.mat2d.create();
        const transforms = this.createPathTransformBuffers(pathCount);

        for (const glyph of atlasGlyphs) {
            const pathID = glyph.pathID;
            const atlasOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);

            glmatrix.mat2d.identity(transform);
            glmatrix.mat2d.translate(transform, transform, atlasOrigin);
            glmatrix.mat2d.translate(transform, transform, stemDarkeningOffset);
            glmatrix.mat2d.rotate(transform, transform, rotationAngle);
            glmatrix.mat2d.scale(transform,
                                 transform,
                                 [pixelsPerUnit, pixelsPerUnit * stemDarkeningYScale]);

            transforms.st[pathID * 4 + 0] = transform[0];
            transforms.st[pathID * 4 + 1] = transform[3];
            transforms.st[pathID * 4 + 2] = transform[4];
            transforms.st[pathID * 4 + 3] = transform[5];

            transforms.ext[pathID * 2 + 0] = transform[1];
            transforms.ext[pathID * 2 + 1] = transform[2];
        }

        return transforms;
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

    protected clearForDirectRendering(): void {}

    protected buildAtlasGlyphs(atlasGlyphs: AtlasGlyph[]): void {
        const renderContext = this.renderContext;
        const font = renderContext.font;
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;
        const hint = this.createHint();

        atlasGlyphs.sort((a, b) => a.glyphKey.sortKey - b.glyphKey.sortKey);
        atlasGlyphs = _.sortedUniqBy(atlasGlyphs, glyph => glyph.glyphKey.sortKey);
        if (atlasGlyphs.length === 0)
            return;

        renderContext.atlasGlyphs = atlasGlyphs;
        renderContext.atlas.layoutGlyphs(atlasGlyphs,
                                         font,
                                         pixelsPerUnit,
                                         rotationAngle,
                                         hint,
                                         this.emboldenAmount);

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

    protected newTimingsReceived(): void {
        this.renderContext.newTimingsReceived(this.lastTimings);
    }

    protected createHint(): Hint {
        return new Hint(this.renderContext.font,
                        this.pixelsPerUnit,
                        this.renderContext.useHinting);
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}

export interface SimpleTextLayoutRenderContext extends TextRenderContext {
    readonly layout: SimpleTextLayout;
    readonly rotationAngle: number;
    readonly emboldenAmount: number;
    readonly unitsPerEm: number;
}

export abstract class SimpleTextLayoutRenderer extends TextRenderer {
    abstract get renderContext(): SimpleTextLayoutRenderContext;

    glyphPositionsBuffer!: WebGLBuffer;
    glyphTexCoordsBuffer!: WebGLBuffer;
    glyphElementsBuffer!: WebGLBuffer;

    private glyphBounds!: Float32Array;

    get layout(): SimpleTextLayout {
        return this.renderContext.layout;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.create();
    }

    get rotationAngle(): number {
        return this.renderContext.rotationAngle;
    }

    protected get extraEmboldenAmount(): glmatrix.vec2 {
        const emboldenLength = this.renderContext.emboldenAmount * this.renderContext.unitsPerEm;
        return glmatrix.vec2.clone([emboldenLength, emboldenLength]);
    }

    prepareToAttachText(): void {
        if (this.atlasFramebuffer == null)
            this.createAtlasFramebuffer();

        this.layoutText();
    }

    finishAttachingText(): void {
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           aaOptions: AAOptions):
                           void {
        super.setAntialiasingOptions(aaType, aaLevel, aaOptions);

        // Need to relayout because changing AA options can cause font dilation to change...
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    relayoutText(): void {
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    updateHinting(): void {
        // Need to relayout the text because the pixel bounds of the glyphs can change from this...
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    updateEmboldenAmount(): void {
        // Likewise, need to relayout the text.
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    viewPanned(): void {
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    protected compositeIfNecessary(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        // Set up composite state.
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gl.viewport(0, 0, renderContext.cameraView.width, renderContext.cameraView.height);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.SCISSOR_TEST);
        gl.blendEquation(gl.FUNC_REVERSE_SUBTRACT);
        gl.blendFuncSeparate(gl.ONE, gl.ONE, gl.ZERO, gl.ONE);
        gl.enable(gl.BLEND);

        // Clear.
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        // Set the appropriate program.
        const programName = this.gammaCorrectionMode === 'off' ? 'blitLinear' : 'blitGamma';
        const blitProgram = this.renderContext.shaderPrograms[programName];

        // Set up the composite VAO.
        const attributes = blitProgram.attributes;
        gl.useProgram(blitProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        gl.vertexAttribPointer(attributes.aPosition, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphTexCoordsBuffer);
        gl.vertexAttribPointer(attributes.aTexCoord, 2, gl.FLOAT, false, 0, 0);
        gl.enableVertexAttribArray(attributes.aPosition);
        gl.enableVertexAttribArray(attributes.aTexCoord);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);

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
        gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        gl.activeTexture(gl.TEXTURE0);
        const destTexture = this.renderContext
                                .atlas
                                .ensureTexture(this.renderContext);
        gl.bindTexture(gl.TEXTURE_2D, destTexture);
        gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        this.bindGammaLUT(glmatrix.vec3.clone([1.0, 1.0, 1.0]), 1, blitProgram.uniforms);
        const totalGlyphCount = this.layout.textFrame.totalGlyphCount;
        gl.drawElements(gl.TRIANGLES, totalGlyphCount * 6, gl.UNSIGNED_INT, 0);
    }

    private layoutText(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        this.layout.layoutRuns();

        const textBounds = this.layout.textFrame.bounds;
        this.camera.bounds = textBounds;

        const totalGlyphCount = this.layout.textFrame.totalGlyphCount;
        const glyphPositions = new Float32Array(totalGlyphCount * 8);
        const glyphIndices = new Uint32Array(totalGlyphCount * 6);

        const hint = this.createHint();
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;

        let globalGlyphIndex = 0;
        for (const run of this.layout.textFrame.runs) {
            run.recalculatePixelRects(pixelsPerUnit,
                                      rotationAngle,
                                      hint,
                                      this.emboldenAmount,
                                      SUBPIXEL_GRANULARITY,
                                      textBounds);

            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const rect = run.pixelRectForGlyphAt(glyphIndex);
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

        this.glyphPositionsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, glyphPositions, gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, glyphIndices, gl.STATIC_DRAW);
    }

    private buildGlyphs(): void {
        const font = this.renderContext.font;
        const glyphStore = this.renderContext.glyphStore;
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;

        const textFrame = this.layout.textFrame;
        const textBounds = textFrame.bounds;
        const hint = this.createHint();

        // Only build glyphs in view.
        const translation = this.camera.translation;
        const canvasRect = glmatrix.vec4.clone([
            -translation[0],
            -translation[1],
            -translation[0] + this.renderContext.cameraView.width,
            -translation[1] + this.renderContext.cameraView.height,
        ]);

        const atlasGlyphs = [];
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                const pixelRect = run.pixelRectForGlyphAt(glyphIndex);
                if (!rectsIntersect(pixelRect, canvasRect))
                    continue;

                const glyphID = run.glyphIDs[glyphIndex];
                const glyphStoreIndex = glyphStore.indexOfGlyphWithID(glyphID);
                if (glyphStoreIndex == null)
                    continue;

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        pixelsPerUnit,
                                                        rotationAngle,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY,
                                                        textBounds);
                const glyphKey = new GlyphKey(glyphID, subpixel);
                atlasGlyphs.push(new AtlasGlyph(glyphStoreIndex, glyphKey));
            }
        }

        this.buildAtlasGlyphs(atlasGlyphs);

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.
        this.setGlyphTexCoords();
    }

    private setGlyphTexCoords(): void {
        const gl = this.renderContext.gl;

        const textFrame = this.layout.textFrame;
        const textBounds = textFrame.bounds;

        const font = this.renderContext.font;
        const atlasGlyphs = this.renderContext.atlasGlyphs;

        const hint = this.createHint();
        const pixelsPerUnit = this.pixelsPerUnit;
        const rotationAngle = this.rotationAngle;

        const atlasGlyphKeys = atlasGlyphs.map(atlasGlyph => atlasGlyph.glyphKey.sortKey);

        this.glyphBounds = new Float32Array(textFrame.totalGlyphCount * 8);

        let globalGlyphIndex = 0;
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const textGlyphID = run.glyphIDs[glyphIndex];

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        pixelsPerUnit,
                                                        rotationAngle,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY,
                                                        textBounds);

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
                                                              rotationAngle,
                                                              this.emboldenAmount);

                const atlasGlyphPixelOrigin =
                    atlasGlyph.calculateSubpixelOrigin(pixelsPerUnit);
                const atlasGlyphRect = calculatePixelRectForGlyph(atlasGlyphUnitMetrics,
                                                                  atlasGlyphPixelOrigin,
                                                                  pixelsPerUnit,
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

        this.glyphTexCoordsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphTexCoordsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, this.glyphBounds, gl.STATIC_DRAW);
    }

    private setIdentityTexScaleUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform2f(uniforms.uTexScale, 1.0, 1.0);
    }
}

/// The separating axis theorem.
function rectsIntersect(a: glmatrix.vec4, b: glmatrix.vec4): boolean {
    return a[2] > b[0] && a[3] > b[1] && a[0] < b[2] && a[1] < b[3];
}
