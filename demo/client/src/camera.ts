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

const SCALE_FACTOR: number = 1.0 / 100.0;

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

            this.scale *= 1.0 - event.deltaY * window.devicePixelRatio * SCALE_FACTOR;

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
