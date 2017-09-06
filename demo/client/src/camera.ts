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
import {PathfinderView} from "./view";

const ORTHOGRAPHIC_ZOOM_SPEED: number = 1.0 / 100.0;

const PERSPECTIVE_MOVEMENT_SPEED: number = 10.0;
const PERSPECTIVE_ROTATION_SPEED: number = 1.0 / 300.0;

const MOVEMENT_INTERVAL_DELAY: number = 10;

const INITIAL_TRANSLATION: glmatrix.vec3 = glmatrix.vec3.fromValues(0.0, 0.0, -1000.0);

export abstract class Camera {
    constructor(canvas: HTMLCanvasElement) {
        this.canvas = canvas;
    }

    protected canvas: HTMLCanvasElement;
}

export class OrthographicCamera extends Camera {
    constructor(canvas: HTMLCanvasElement) {
        super(canvas);

        this.translation = glmatrix.vec2.create();
        this.scale = 1.0;

        this.canvas.addEventListener('wheel', event => this.onWheel(event), false);

        this.onPan = null;
        this.onZoom = null;
    }

    onWheel(event: MouseWheelEvent) {
        event.preventDefault();

        if (event.ctrlKey) {
            // Zoom event: see https://developer.mozilla.org/en-US/docs/Web/Events/wheel
            const mouseLocation = glmatrix.vec2.fromValues(event.clientX, event.clientY);
            const canvasLocation = this.canvas.getBoundingClientRect();
            mouseLocation[0] -= canvasLocation.left;
            mouseLocation[1] = canvasLocation.bottom - mouseLocation[1];
            glmatrix.vec2.scale(mouseLocation, mouseLocation, window.devicePixelRatio);

            const absoluteTranslation = glmatrix.vec2.create();
            glmatrix.vec2.sub(absoluteTranslation, this.translation, mouseLocation);
            glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, 1.0 / this.scale);

            this.scale *= 1.0 - event.deltaY * window.devicePixelRatio * ORTHOGRAPHIC_ZOOM_SPEED;

            glmatrix.vec2.scale(absoluteTranslation, absoluteTranslation, this.scale);
            glmatrix.vec2.add(this.translation, absoluteTranslation, mouseLocation);

            if (this.onZoom != null)
                this.onZoom();
        } else {
            // Pan event.
            const delta = glmatrix.vec2.fromValues(-event.deltaX, event.deltaY);
            glmatrix.vec2.scale(delta, delta, window.devicePixelRatio);
            glmatrix.vec2.add(this.translation, this.translation, delta);

            if (this.onPan != null)
                this.onPan();
        }
    }

    onPan: (() => void) | null;
    onZoom: (() => void) | null;

    translation: glmatrix.vec2;
    scale: number;
}

export class PerspectiveCamera extends Camera {
    constructor(canvas: HTMLCanvasElement) {
        super(canvas);

        this.translation = glmatrix.vec3.clone(INITIAL_TRANSLATION);
        this.rotation = glmatrix.vec2.create();
        this.movementDelta = glmatrix.vec3.create();
        this.movementInterval = null;

        this.canvas.addEventListener('mousedown', event => this.onMouseDown(event), false);
        this.canvas.addEventListener('mouseup', event => this.onMouseUp(event), false);
        this.canvas.addEventListener('mousemove', event => this.onMouseMove(event), false);

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

        if (this.movementInterval == null)
            this.movementInterval = window.setInterval(() => this.move(), MOVEMENT_INTERVAL_DELAY);
    }

    private onMouseUp(event: MouseEvent): void {
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

    private onMouseMove(event: MouseEvent): void {
        if (document.pointerLockElement !== this.canvas)
            return;

        this.rotation[1] += event.movementX * PERSPECTIVE_ROTATION_SPEED;
        this.rotation[0] += event.movementY * PERSPECTIVE_ROTATION_SPEED;

        if (this.onChange != null)
            this.onChange();
    }

    get rotationMatrix(): glmatrix.mat4 {
        const matrix = glmatrix.mat4.create();
        glmatrix.mat4.fromYRotation(matrix, this.rotation[1]);
        glmatrix.mat4.rotateX(matrix, matrix, this.rotation[0]);
        return matrix;
    }

    onChange: (() => void) | null;

    translation: glmatrix.vec3;

    /// Pitch and yaw Euler angles.
    rotation: glmatrix.vec2;

    private movementDelta: glmatrix.vec3;
    private movementInterval: number | null;
}
