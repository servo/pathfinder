// pathfinder/client/src/camera.ts
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

import {PathfinderView} from "./view";

const ORTHOGRAPHIC_ZOOM_SPEED: number = 1.0 / 100.0;

const ORTHOGRAPHIC_ZOOM_IN_FACTOR: number = 1.2;
const ORTHOGRAPHIC_ZOOM_OUT_FACTOR: number = 1.0 / ORTHOGRAPHIC_ZOOM_IN_FACTOR;

const ORTHOGRAPHIC_DEFAULT_MIN_SCALE: number = 0.01;
const ORTHOGRAPHIC_DEFAULT_MAX_SCALE: number = 1000.0;

const PERSPECTIVE_MOVEMENT_SPEED: number = 10.0;
const PERSPECTIVE_ROTATION_SPEED: number = 1.0 / 300.0;

const PERSPECTIVE_MOVEMENT_VECTORS: PerspectiveMovementVectors = _.fromPairs([
    ['W'.charCodeAt(0), glmatrix.vec3.fromValues(0, 0,  PERSPECTIVE_MOVEMENT_SPEED)],
    ['A'.charCodeAt(0), glmatrix.vec3.fromValues(PERSPECTIVE_MOVEMENT_SPEED, 0, 0)],
    ['S'.charCodeAt(0), glmatrix.vec3.fromValues(0, 0, -PERSPECTIVE_MOVEMENT_SPEED)],
    ['D'.charCodeAt(0), glmatrix.vec3.fromValues(-PERSPECTIVE_MOVEMENT_SPEED, 0, 0)],
]);

const PERSPECTIVE_MOVEMENT_INTERVAL_DELAY: number = 10;

const PERSPECTIVE_INITIAL_TRANSLATION: glmatrix.vec3 =
    glmatrix.vec3.clone([1750.0, 700.0, -1750.0]);
const PERSPECTIVE_INITIAL_ROTATION: glmatrix.vec2 = glmatrix.vec2.clone([Math.PI * 0.25, 0.0]);

interface PerspectiveMovementVectors {
    [keyCode: number]: glmatrix.vec3;
}

export abstract class Camera {
    constructor(canvas: HTMLCanvasElement) {
        this.canvas = canvas;
    }

    abstract zoomIn(): void;
    abstract zoomOut(): void;

    protected canvas: HTMLCanvasElement;
}

export class OrthographicCamera extends Camera {
    constructor(canvas: HTMLCanvasElement, minScale?: number, maxScale?: number) {
        super(canvas);

        this.minScale = _.defaultTo(minScale, ORTHOGRAPHIC_DEFAULT_MIN_SCALE);
        this.maxScale = _.defaultTo(maxScale, ORTHOGRAPHIC_DEFAULT_MAX_SCALE);

        this.translation = glmatrix.vec2.create();
        this.scale = 1.0;

        this._bounds = glmatrix.vec4.create();

        this.canvas.addEventListener('wheel', event => this.onWheel(event), false);
        this.canvas.addEventListener('mousedown', event => this.onMouseDown(event), false);
        this.canvas.addEventListener('mouseup', event => this.onMouseUp(event), false);
        this.canvas.addEventListener('mousemove', event => this.onMouseMove(event), false);

        this.onPan = null;
        this.onZoom = null;
    }

    onWheel(event: MouseWheelEvent): void {
        event.preventDefault();

        if (!event.ctrlKey) {
            this.pan(glmatrix.vec2.fromValues(-event.deltaX, event.deltaY));
            return;
        }

        // Zoom event: see https://developer.mozilla.org/en-US/docs/Web/Events/wheel
        const mouseLocation = glmatrix.vec2.fromValues(event.clientX, event.clientY);
        const canvasLocation = this.canvas.getBoundingClientRect();
        mouseLocation[0] -= canvasLocation.left;
        mouseLocation[1] = canvasLocation.bottom - mouseLocation[1];
        glmatrix.vec2.scale(mouseLocation, mouseLocation, window.devicePixelRatio);

        const scale = 1.0 - event.deltaY * window.devicePixelRatio * ORTHOGRAPHIC_ZOOM_SPEED;
        this.zoom(scale, mouseLocation);
    }

    private onMouseDown(event: MouseEvent): void {
        this.canvas.classList.add('pf-grabbing');
    }

    private onMouseUp(event: MouseEvent): void {
        this.canvas.classList.remove('pf-grabbing');
    }

    private onMouseMove(event: MouseEvent): void {
        if ((event.buttons & 1) !== 0)
            this.pan(glmatrix.vec2.fromValues(event.movementX, -event.movementY));
    }

    private pan(delta: glmatrix.vec2): void {
        // Pan event.
        glmatrix.vec2.scale(delta, delta, window.devicePixelRatio);
        glmatrix.vec2.add(this.translation, this.translation, delta);

        this.clampViewport();

        if (this.onPan != null)
            this.onPan();
    }

    private clampViewport() {
        const bounds = glmatrix.vec4.clone(this._bounds);
        for (let axis = 0; axis < 2; axis++) {
            const viewportLength = axis === 0 ? this.canvas.width : this.canvas.height;
            const axisBounds = [bounds[axis + 0], bounds[axis + 2]];
            const boundsLength = axisBounds[1] - axisBounds[0];
            if (viewportLength < boundsLength) {
                // Viewport must be inside bounds.
                this.translation[axis] = _.clamp(this.translation[axis],
                                                 viewportLength - axisBounds[1],
                                                 -axisBounds[0]);
            } else {
                // Bounds must be inside viewport.
                this.translation[axis] = _.clamp(this.translation[axis],
                                                 -axisBounds[0],
                                                 viewportLength - axisBounds[1]);
            }
        }
    }

    zoomToFit(): void {
        const upperLeft = glmatrix.vec2.fromValues(this._bounds[0], this._bounds[1]);
        const lowerRight = glmatrix.vec2.fromValues(this._bounds[2], this._bounds[3]);
        const width = this._bounds[2] - this._bounds[0];
        const height = Math.abs(this._bounds[1] - this._bounds[3]);

        // Scale appropriately.
        this.scale = Math.min(this.canvas.width / width, this.canvas.height / height);

        // Center.
        this.translation = glmatrix.vec2.create();
        glmatrix.vec2.lerp(this.translation, upperLeft, lowerRight, 0.5);
        glmatrix.vec2.scale(this.translation, this.translation, -this.scale);
        this.translation[0] += this.canvas.width * 0.5;
        this.translation[1] += this.canvas.height * 0.5;

        if (this.onZoom != null)
            this.onZoom();
        if (this.onPan != null)
            this.onPan();
    }

    zoomIn(): void {
        this.zoom(ORTHOGRAPHIC_ZOOM_IN_FACTOR, this.centerPoint);
    }

    zoomOut(): void {
        this.zoom(ORTHOGRAPHIC_ZOOM_OUT_FACTOR, this.centerPoint);
    }

    private zoom(scale: number, point: glmatrix.vec2): void {
        const absoluteTranslation = glmatrix.vec2.create();
        glmatrix.vec2.sub(absoluteTranslation, this.translation, point);
        glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, 1.0 / this.scale);

        this.scale = _.clamp(this.scale * scale, this.minScale, this.maxScale);

        glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, this.scale);
        glmatrix.vec2.add(this.translation, absoluteTranslation, point);

        this.clampViewport();

        if (this.onZoom != null)
            this.onZoom();
    }

    private get centerPoint(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width * 0.5, this.canvas.height * 0.5);
    }

    get bounds(): glmatrix.vec4 {
        return this._bounds;
    }

    set bounds(newBounds: glmatrix.vec4) {
        this._bounds = glmatrix.vec4.clone(newBounds);
    }

    onPan: (() => void) | null;
    onZoom: (() => void) | null;

    private _bounds: glmatrix.vec4;

    translation: glmatrix.vec2;
    scale: number;

    private readonly minScale: number;
    private readonly maxScale: number;
}

export class PerspectiveCamera extends Camera {
    constructor(canvas: HTMLCanvasElement) {
        super(canvas);

        this.translation = glmatrix.vec3.clone(PERSPECTIVE_INITIAL_TRANSLATION);
        this.rotation = glmatrix.vec2.clone(PERSPECTIVE_INITIAL_ROTATION);
        this.movementDelta = glmatrix.vec3.create();
        this.movementInterval = null;

        this.canvas.addEventListener('mousedown', event => this.onMouseDown(event), false);
        this.canvas.addEventListener('mouseup', event => this.onMouseUp(event), false);
        this.canvas.addEventListener('mousemove', event => this.onMouseMove(event), false);

        window.addEventListener('keydown', event => this.onKeyDown(event), false);
        window.addEventListener('keyup', event => this.onKeyUp(event), false);

        this.onChange = null;
    }

    private onMouseDown(event: MouseEvent): void {
        if (document.pointerLockElement !== this.canvas) {
            this.canvas.requestPointerLock();
            return;
        }

        this.movementDelta = glmatrix.vec3.fromValues(0.0, 0.0, PERSPECTIVE_MOVEMENT_SPEED);
        if (event.button !== 1)
            this.movementDelta[0] = -this.movementDelta[0];

        this.startMoving();
    }

    private onMouseUp(event: MouseEvent): void {
        this.stopMoving();
    }

    private onMouseMove(event: MouseEvent): void {
        if (document.pointerLockElement !== this.canvas)
            return;

        this.rotation[0] += event.movementX * PERSPECTIVE_ROTATION_SPEED;
        this.rotation[1] += event.movementY * PERSPECTIVE_ROTATION_SPEED;

        if (this.onChange != null)
            this.onChange();
    }

    private onKeyDown(event: KeyboardEvent): void {
        if (PERSPECTIVE_MOVEMENT_VECTORS.hasOwnProperty(event.keyCode)) {
            this.movementDelta = glmatrix.vec3.clone(PERSPECTIVE_MOVEMENT_VECTORS[event.keyCode]);
            this.startMoving();
        }
    }

    private onKeyUp(event: KeyboardEvent): void {
        if (PERSPECTIVE_MOVEMENT_VECTORS.hasOwnProperty(event.keyCode))
            this.stopMoving();
    }

    private startMoving(): void {
        if (this.movementInterval == null)
            this.movementInterval = window.setInterval(() => this.move(), PERSPECTIVE_MOVEMENT_INTERVAL_DELAY);
    }

    private stopMoving(): void {
        if (this.movementInterval != null) {
            window.clearInterval(this.movementInterval);
            this.movementInterval = null;
            this.movementDelta = glmatrix.vec3.create();
        }
    }

    private move() {
        const invRotationMatrix = glmatrix.mat4.create();
        glmatrix.mat4.invert(invRotationMatrix, this.rotationMatrix);

        const delta = glmatrix.vec3.clone(this.movementDelta);
        glmatrix.vec3.transformMat4(delta, delta, invRotationMatrix);
        glmatrix.vec3.add(this.translation, this.translation, delta);

        if (this.onChange != null)
            this.onChange();
    }

    get rotationMatrix(): glmatrix.mat4 {
        const matrix = glmatrix.mat4.create();
        glmatrix.mat4.fromXRotation(matrix, this.rotation[1]);
        glmatrix.mat4.rotateY(matrix, matrix, this.rotation[0]);
        return matrix;
    }

    zoomIn(): void {
        // TODO(pcwalton)
    }

    zoomOut(): void {
        // TODO(pcwalton)
    }

    onChange: (() => void) | null;

    translation: glmatrix.vec3;

    /// Yaw and pitch Euler angles.
    rotation: glmatrix.vec2;

    private movementDelta: glmatrix.vec3;
    private movementInterval: number | null;
}
