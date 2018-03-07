// pathfinder/demo/client/src/svg-renderer.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, AntialiasingStrategyName, DirectRenderingMode} from './aa-strategy';
import {NoAAStrategy} from './aa-strategy';
import {SubpixelAAType} from './aa-strategy';
import {OrthographicCamera} from "./camera";
import {UniformMap} from './gl-utils';
import {PathfinderPackedMeshes} from './meshes';
import {PathTransformBuffers, Renderer} from "./renderer";
import {ShaderMap} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {SVGLoader} from './svg-loader';
import {Range} from './utils';
import {RenderContext} from './view';
import {MCAAStrategy, XCAAStrategy} from './xcaa-strategy';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof MCAAStrategy;
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: MCAAStrategy,
};

export interface SVGRendererOptions {
    sizeToFit?: boolean;
    fixed?: boolean;
}

export abstract class SVGRenderer extends Renderer {
    renderContext!: RenderContext;

    camera: OrthographicCamera;

    needsStencil: boolean = false;

    private options: SVGRendererOptions;

    get isMulticolor(): boolean {
        return !this.loader.isMonochrome;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);
    }

    get destAllocatedSize(): glmatrix.vec2 {
        const canvas = this.canvas;
        return glmatrix.vec2.clone([canvas.width, canvas.height]);
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    protected get objectCount(): number {
        return 1;
    }

    protected abstract get loader(): SVGLoader;
    protected abstract get canvas(): HTMLCanvasElement;

    constructor(renderContext: RenderContext, options: SVGRendererOptions) {
        super(renderContext);

        this.options = options;

        // FIXME(pcwalton): Get the canvas a better way?
        this.camera = new OrthographicCamera((this as any).canvas, {
            fixed: !!this.options.fixed,
            scaleBounds: true,
        });

        this.camera.onPan = () => this.renderContext.setDirty();
        this.camera.onZoom = () => this.renderContext.setDirty();
        this.camera.onRotate = () => this.renderContext.setDirty();
    }

    setHintsUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform4f(uniforms.uHints, 0, 0, 0, 0);
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const loader = this.loader;
        const boundingRectsBuffer = new Float32Array((loader.pathBounds.length + 1) * 4);
        for (let pathIndex = 0; pathIndex < loader.pathBounds.length; pathIndex++)
            boundingRectsBuffer.set(loader.pathBounds[pathIndex], (pathIndex + 1) * 4);
        return boundingRectsBuffer;
    }

    attachMeshes(meshes: PathfinderPackedMeshes[]): void {
        super.attachMeshes(meshes);
        this.uploadPathColors(1);
        this.uploadPathTransforms(1);
    }

    initCameraBounds(svgViewBox: glmatrix.vec4): void {
        // The SVG origin is in the upper left, but the camera origin is in the lower left.
        this.camera.bounds = svgViewBox;

        if (this.options.sizeToFit)
            this.camera.zoomToFit();
    }

    meshIndexForObject(objectIndex: number): number {
        return 0;
    }

    pathRangeForObject(objectIndex: number): Range {
        return new Range(1, this.loader.pathInstances.length + 1);
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.clone([1.0, 1.0]);
    }

    protected get worldTransform(): glmatrix.mat4 {
        const canvas = this.canvas;

        const transform = glmatrix.mat4.create();

        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / canvas.width, 2.0 / canvas.height, 1.0]);

        const centerPoint = glmatrix.vec3.clone([canvas.width * 0.5, canvas.height * 0.5, 0.0]);
        glmatrix.mat4.translate(transform, transform, centerPoint);
        glmatrix.mat4.rotateZ(transform, transform, this.camera.rotationAngle);
        glmatrix.vec3.negate(centerPoint, centerPoint);
        glmatrix.mat4.translate(transform, transform, centerPoint);

        const translation = this.camera.translation;
        glmatrix.mat4.translate(transform, transform, [translation[0], translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.camera.scale, this.camera.scale, 1.0]);
        return transform;
    }

    protected clearColorForObject(objectIndex: number): glmatrix.vec4 | null {
        return glmatrix.vec4.create();
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(renderingMode: DirectRenderingMode):
                                        keyof ShaderMap<void> {
        return renderingMode === 'conservative' ? 'conservativeInterior' : 'directInterior';
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const instances = this.loader.pathInstances;
        const pathColors = new Uint8Array(4 * (instances.length + 1));

        for (let pathIndex = 0; pathIndex < instances.length; pathIndex++) {
            const startOffset = (pathIndex + 1) * 4;

            // Set color.
            const color: ArrayLike<number> = instances[pathIndex].color;
            pathColors.set(instances[pathIndex].color, startOffset);
            pathColors[startOffset + 3] = color[3] * 255;
        }

        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array> {
        const instances = this.loader.pathInstances;
        const pathTransforms = this.createPathTransformBuffers(instances.length);

        for (let pathIndex = 0; pathIndex < instances.length; pathIndex++) {
            // TODO(pcwalton): Set transform.
            const startOffset = (pathIndex + 1) * 4;
            pathTransforms.st.set([1, 1, 0, 0], startOffset);
        }

        return pathTransforms;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {}
}
