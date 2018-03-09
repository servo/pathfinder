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

import {EPSILON, unwrapNull} from "./utils";
import {PathfinderView} from "./view";

const PIXELS_PER_LINE: number = 16.0;

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

const PERSPECTIVE_OUTER_COLLISION_EXTENT: number = 3000.0;
const PERSPECTIVE_HITBOX_RADIUS: number = 1.0;

const KEYCODES = ["W", "A", "S", "D"].map(x => x.charCodeAt(0));

export interface OrthographicCameraOptions {
    fixed?: boolean;
    minScale?: number;
    maxScale?: number;
    scaleBounds?: boolean;
    ignoreBounds?: boolean;
}

export interface PerspectiveCameraOptions {
    innerCollisionExtent?: number;
}

interface PerspectiveMovementVectors {
    [keyCode: number]: glmatrix.vec3;
}

interface PerspectiveMovementKeys {
    [keyCode: number]: boolean;
}

export interface CameraView {
    readonly width: number;
    readonly height: number;
    readonly classList: DOMTokenList | null;

    addEventListener<K extends keyof HTMLElementEventMap>(type: K,
                                                          listener: (this: HTMLCanvasElement,
                                                                     ev: HTMLElementEventMap[K]) =>
                                                                     any,
                                                          useCapture?: boolean): void;
    getBoundingClientRect(): ClientRect;
}

export abstract class Camera {
    protected canvas: CameraView;

    constructor(canvas: CameraView) {
        this.canvas = canvas;
    }

    abstract zoom(scale: number): void;
    abstract zoomIn(): void;
    abstract zoomOut(): void;
    abstract rotate(newAngle: number): void;
}

export class OrthographicCamera extends Camera {
    onPan: (() => void) | null;
    onZoom: (() => void) | null;
    onRotate: (() => void) | null;

    translation!: glmatrix.vec2;
    scale!: number;
    rotationAngle!: number;

    private _bounds: glmatrix.vec4;

    private readonly fixed: boolean;
    private readonly minScale: number;
    private readonly maxScale: number;
    private readonly scaleBounds: boolean;
    private readonly ignoreBounds: boolean;

    constructor(canvas: CameraView, options?: OrthographicCameraOptions) {
        super(canvas);

        if (options == null)
            options = {};

        this.fixed = !!options.fixed;
        this.minScale = _.defaultTo(options.minScale, ORTHOGRAPHIC_DEFAULT_MIN_SCALE);
        this.maxScale = _.defaultTo(options.maxScale, ORTHOGRAPHIC_DEFAULT_MAX_SCALE);
        this.scaleBounds = !!options.scaleBounds;
        this.ignoreBounds = !!options.ignoreBounds;

        this.reset();

        this._bounds = glmatrix.vec4.create();

        if (!this.fixed) {
            this.canvas.addEventListener('wheel', event => this.onWheel(event), false);
            this.canvas.addEventListener('mousedown', event => this.onMouseDown(event), false);
            this.canvas.addEventListener('mouseup', event => this.onMouseUp(event), false);
            this.canvas.addEventListener('mousemove', event => this.onMouseMove(event), false);
            if (this.canvas.classList != null)
                this.canvas.classList.add('pf-draggable');
        } else {
            if (this.canvas.classList != null)
                this.canvas.classList.remove('pf-draggable');
        }

        this.onPan = null;
        this.onZoom = null;
        this.onRotate = null;
    }

    onWheel(event: MouseWheelEvent): void {
        if (this.canvas == null)
            throw new Error("onWheel() with no canvas?!");

        event.preventDefault();

        if (!event.ctrlKey) {
            const delta = glmatrix.vec2.fromValues(-event.deltaX, event.deltaY);
            if (event.deltaMode === event.DOM_DELTA_LINE)
                glmatrix.vec2.scale(delta, delta, PIXELS_PER_LINE);
            this.pan(delta);
            return;
        }

        // Zoom event: see https://developer.mozilla.org/en-US/docs/Web/Events/wheel
        const mouseLocation = glmatrix.vec2.fromValues(event.clientX, event.clientY);
        const canvasLocation = this.canvas.getBoundingClientRect();
        mouseLocation[0] -= canvasLocation.left;
        mouseLocation[1] = canvasLocation.bottom - mouseLocation[1];
        glmatrix.vec2.scale(mouseLocation, mouseLocation, window.devicePixelRatio);

        const scale = 1.0 - event.deltaY * window.devicePixelRatio * ORTHOGRAPHIC_ZOOM_SPEED;
        this.doZoom(scale, mouseLocation);
    }

    zoomToFit(): void {
        const size = this.objectSize();
        this.scale = Math.min(this.canvas.width / size[0], this.canvas.height / size[1]);
        this.center();
    }

    center(): void {
        const upperLeft = glmatrix.vec2.clone([this._bounds[0], this._bounds[1]]);
        const lowerRight = glmatrix.vec2.clone([this._bounds[2], this._bounds[3]]);

        this.translation = glmatrix.vec2.create();
        glmatrix.vec2.lerp(this.translation, upperLeft, lowerRight, 0.5);
        glmatrix.vec2.scale(this.translation, this.translation, -this.scale);
        this.translation[0] += this.canvas.width * 0.5;
        this.translation[1] += this.canvas.height * 0.5;
    }

    zoomToSize(newSize: number): void {
        this.reset();

        const size = this.objectSize();
        const length = Math.max(size[0], size[1]);
        this.zoom(newSize / length);
    }

    zoom(scale: number): void {
        this.doZoom(scale, this.centerPoint);
    }

    zoomIn(): void {
        this.doZoom(ORTHOGRAPHIC_ZOOM_IN_FACTOR, this.centerPoint);
    }

    zoomOut(): void {
        this.doZoom(ORTHOGRAPHIC_ZOOM_OUT_FACTOR, this.centerPoint);
    }

    rotate(newAngle: number): void {
        this.rotationAngle = newAngle;

        if (this.onRotate != null)
            this.onRotate();
    }

    private objectSize(): glmatrix.vec2 {
        const upperLeft = glmatrix.vec2.clone([this._bounds[0], this._bounds[1]]);
        const lowerRight = glmatrix.vec2.clone([this._bounds[2], this._bounds[3]]);
        const width = this._bounds[2] - this._bounds[0];
        const height = Math.abs(this._bounds[1] - this._bounds[3]);
        return glmatrix.vec2.clone([width, height]);
    }

    private reset(): void {
        this.translation = glmatrix.vec2.create();
        this.scale = 1.0;
        this.rotationAngle = 0.0;
    }

    private onMouseDown(event: MouseEvent): void {
        if (this.canvas.classList != null)
            this.canvas.classList.add('pf-grabbing');
    }

    private onMouseUp(event: MouseEvent): void {
        if (this.canvas.classList != null)
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
        if (this.ignoreBounds)
            return;

        const bounds = glmatrix.vec4.clone(this.bounds);
        if (!this.scaleBounds)
            glmatrix.vec4.scale(bounds, bounds, this.scale);

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

    private doZoom(scale: number, point: glmatrix.vec2): void {
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
        return glmatrix.vec2.clone([this.canvas.width * 0.5, this.canvas.height * 0.5]);
    }

    get bounds(): glmatrix.vec4 {
        const bounds = glmatrix.vec4.clone(this._bounds);
        if (this.scaleBounds)
            glmatrix.vec4.scale(bounds, bounds, this.scale);
        return bounds;
    }

    set bounds(newBounds: glmatrix.vec4) {
        this._bounds = glmatrix.vec4.clone(newBounds);
    }
}

export class PerspectiveCamera extends Camera {
    canvas!: HTMLCanvasElement;

    onChange: (() => void) | null;

    translation: glmatrix.vec3;

    /// Yaw and pitch Euler angles.
    rotation: glmatrix.vec2;

    private movementDelta: glmatrix.vec3;
    // If W, A, S, D are pressed
    private wasdPress: PerspectiveMovementKeys;
    private movementInterval: number | null;

    private readonly innerCollisionExtent: number;

    private vrRotationMatrix: glmatrix.mat4 | null;

    constructor(canvas: HTMLCanvasElement, options?: PerspectiveCameraOptions) {
        super(canvas);

        if (options == null)
            options = {};
        this.innerCollisionExtent = options.innerCollisionExtent || 0.0;

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
        this.vrRotationMatrix = null;

        this.wasdPress = _.fromPairs([
            ['W'.charCodeAt(0), false],
            ['A'.charCodeAt(0), false],
            ['S'.charCodeAt(0), false],
            ['D'.charCodeAt(0), false],
        ]);
    }

    zoom(scale: number): void {
        // TODO(pcwalton)
    }

    zoomIn(): void {
        // TODO(pcwalton)
    }

    zoomOut(): void {
        // TODO(pcwalton)
    }

    rotate(newAngle: number): void {
        // TODO(pcwalton)
    }

    setView(rotation: glmatrix.mat4, pose: VRPose) {
        this.vrRotationMatrix = rotation;
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

        const trialYRotation = this.rotation[1] + event.movementY * PERSPECTIVE_ROTATION_SPEED;
        this.rotation[1] = _.clamp(trialYRotation, -Math.PI * 0.5, Math.PI * 0.5);

        if (this.onChange != null)
            this.onChange();
    }

    private onKeyDown(event: KeyboardEvent): void {
        if (PERSPECTIVE_MOVEMENT_VECTORS.hasOwnProperty(event.keyCode)) {
            // keyDown will be repeated on prolonged holds of the key,
            // don't do extra computation in that case
            if (this.wasdPress[event.keyCode])
                return;
            this.wasdPress[event.keyCode] = true;
            this.updateMovementDelta();
            this.startMoving();
        }
    }

    private onKeyUp(event: KeyboardEvent): void {
        if (PERSPECTIVE_MOVEMENT_VECTORS.hasOwnProperty(event.keyCode)) {
            this.wasdPress[event.keyCode] = false;
            if (this.updateMovementDelta()) {
               this.stopMoving();
           }
        }
    }

    // Updates the movementDelta vector based on what keys are currently pressed
    // Returns true if the vector is now empty
    private updateMovementDelta(): boolean {
        this.movementDelta = glmatrix.vec3.create();
        let empty = true;
        for (const key of KEYCODES) {
            if (this.wasdPress[key]) {
                glmatrix.vec3.add(this.movementDelta,
                                  this.movementDelta,
                                  PERSPECTIVE_MOVEMENT_VECTORS[key]);
                empty = false;
            }
        }
        return empty;
    }

    private startMoving(): void {
        if (this.movementInterval == null)
            this.movementInterval = window.setInterval(() => this.move(),
                                                       PERSPECTIVE_MOVEMENT_INTERVAL_DELAY);
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

        const trialTranslation = glmatrix.vec3.create();
        glmatrix.vec3.add(trialTranslation, this.translation, delta);

        // TODO(pcwalton): Sliding…
        const absoluteTrialTranslationX = Math.abs(trialTranslation[0]);
        const absoluteTrialTranslationZ = Math.abs(trialTranslation[2]);
        if (absoluteTrialTranslationX < this.innerCollisionExtent + PERSPECTIVE_HITBOX_RADIUS &&
            absoluteTrialTranslationZ < this.innerCollisionExtent + PERSPECTIVE_HITBOX_RADIUS) {
            return;
        }

        if (absoluteTrialTranslationX > PERSPECTIVE_OUTER_COLLISION_EXTENT -
            PERSPECTIVE_HITBOX_RADIUS) {
            trialTranslation[0] = Math.sign(trialTranslation[0]) *
                (PERSPECTIVE_OUTER_COLLISION_EXTENT - PERSPECTIVE_HITBOX_RADIUS);
        }
        if (absoluteTrialTranslationZ > PERSPECTIVE_OUTER_COLLISION_EXTENT -
            PERSPECTIVE_HITBOX_RADIUS) {
            trialTranslation[2] = Math.sign(trialTranslation[2]) *
                (PERSPECTIVE_OUTER_COLLISION_EXTENT - PERSPECTIVE_HITBOX_RADIUS);
        }

        this.translation = trialTranslation;

        if (this.onChange != null)
            this.onChange();
    }

    get rotationMatrix(): glmatrix.mat4 {
        if (this.vrRotationMatrix != null) {
            return this.vrRotationMatrix;
        }
        const matrix = glmatrix.mat4.create();
        glmatrix.mat4.fromXRotation(matrix, this.rotation[1]);
        glmatrix.mat4.rotateY(matrix, matrix, this.rotation[0]);
        return matrix;
    }
}
