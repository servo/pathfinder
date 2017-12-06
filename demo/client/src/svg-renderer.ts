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

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {SubpixelAAType} from './aa-strategy';
import {OrthographicCamera} from "./camera";
import {UniformMap} from './gl-utils';
import {PathfinderMeshData} from './meshes';
import {CompositingOperation, RenderTaskType} from './render-task';
import {BaseRenderer, PathTransformBuffers} from "./renderer";
import {ShaderMap} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {SVGLoader} from './svg-loader';
import {Range} from './utils';
import {RenderContext} from './view';
import {ECAAMulticolorStrategy, XCAAStrategy} from './xcaa-strategy';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof XCAAStrategy;
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: ECAAMulticolorStrategy,
};

export abstract class SVGRenderer extends BaseRenderer {
    renderContext: RenderContext;

    camera: OrthographicCamera;

    get usesSTTransform(): boolean {
        return this.camera.usesSTTransform;
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

    get usesIntermediateRenderTargets(): boolean {
        return true;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    protected get objectCount(): number {
        return this.loader.renderTasks.length;
    }

    protected abstract get loader(): SVGLoader;
    protected abstract get canvas(): HTMLCanvasElement;

    constructor(renderContext: RenderContext) {
        super(renderContext);

        this.camera = new OrthographicCamera(this.canvas, { scaleBounds: true });
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

    attachMeshes(meshes: PathfinderMeshData[]): void {
        super.attachMeshes(meshes);
        this.uploadPathColors(1);
        this.uploadPathTransforms(1);
    }

    initCameraBounds(bounds: glmatrix.vec4): void {
        this.camera.bounds = bounds;
        this.camera.zoomToFit();
    }

    renderTaskTypeForObject(objectIndex: number): RenderTaskType {
        return this.loader.renderTasks[objectIndex].type;
    }

    compositingOperationForObject(objectIndex: number): CompositingOperation | null {
        return this.loader.renderTasks[objectIndex].compositingOperation;
    }

    meshIndexForObject(objectIndex: number): number {
        return 0;
    }

    pathRangeForObject(objectIndex: number): Range {
        return this.loader.renderTasks[objectIndex].instanceIndices;
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
        if (this.antialiasingStrategy instanceof XCAAStrategy)
            return 'xcaaMultiDirectCurve';
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        if (this.antialiasingStrategy instanceof XCAAStrategy)
            return 'xcaaMultiDirectInterior';
        return 'directInterior';
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
