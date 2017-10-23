// pathfinder/client/src/app-controller.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AntialiasingStrategyName, StemDarkeningMode, SubpixelAAType} from "./aa-strategy";
import {FilePickerView} from "./file-picker";
import {ShaderLoader, ShaderMap, ShaderProgramSource} from './shader-loader';
import {expectNotNull, unwrapNull, unwrapUndef} from './utils';
import {DemoView, Timings, TIMINGS} from "./view";

export abstract class AppController {
    protected canvas: HTMLCanvasElement;

    protected screenshotButton: HTMLButtonElement | null;

    start() {
        const canvas = document.getElementById('pf-canvas') as HTMLCanvasElement;
    }

    protected loadInitialFile(builtinFileURI: string) {
        const selectFileElement = document.getElementById('pf-select-file') as
            (HTMLSelectElement | null);
        if (selectFileElement != null) {
            const selectedOption = selectFileElement.selectedOptions[0] as HTMLOptionElement;
            this.fetchFile(selectedOption.value, builtinFileURI);
        } else {
            this.fetchFile(this.defaultFile, builtinFileURI);
        }
    }

    protected fetchFile(file: string, builtinFileURI: string) {
        window.fetch(`${builtinFileURI}/${file}`)
              .then(response => response.arrayBuffer())
              .then(data => this.fileLoaded(data, file));
    }

    protected abstract fileLoaded(data: ArrayBuffer, builtinName: string | null): void;

    protected abstract get defaultFile(): string;
}

export abstract class DemoAppController<View extends DemoView> extends AppController {
    view: Promise<View>;

    protected abstract readonly builtinFileURI: string;

    protected filePickerView: FilePickerView | null;

    protected commonShaderSource: string | null;
    protected shaderSources: ShaderMap<ShaderProgramSource> | null;

    private aaLevelSelect: HTMLSelectElement | null;
    private subpixelAARadioButton: HTMLInputElement | null;
    private stemDarkeningRadioButton: HTMLInputElement | null;
    private fpsLabel: HTMLElement | null;

    constructor() {
        super();
    }

    start() {
        super.start();

        const settingsCard = document.getElementById('pf-settings') as (HTMLElement | null);
        const settingsButton = document.getElementById('pf-settings-button') as
            (HTMLButtonElement | null);
        const settingsCloseButton = document.getElementById('pf-settings-close-button') as
            (HTMLButtonElement | null);

        if (settingsButton != null) {
            settingsButton.addEventListener('click', event => {
                event.stopPropagation();
                unwrapNull(settingsCard).classList.toggle('pf-invisible');
            }, false);
        }
        if (settingsCloseButton != null) {
            settingsCloseButton.addEventListener('click', () => {
                unwrapNull(settingsCard).classList.add('pf-invisible');
            }, false);
        }
        if (settingsCard != null) {
            document.body.addEventListener('click', event => {
                let element = event.target as Element | null;
                while (element != null) {
                    if (element === settingsCard)
                        return;
                    element = element.parentElement;
                }

                settingsCard.classList.add('pf-invisible');
            }, false);
        }

        const screenshotButton = document.getElementById('pf-screenshot-button') as
            HTMLButtonElement | null;
        if (screenshotButton != null) {
            screenshotButton.addEventListener('click', () => {
                this.view.then(view => view.queueScreenshot());
            }, false);
        }

        const zoomInButton = document.getElementById('pf-zoom-in-button') as HTMLButtonElement |
            null;
        if (zoomInButton != null) {
            zoomInButton.addEventListener('click', () => {
                this.view.then(view => view.zoomIn());
            }, false);
        }

        const zoomOutButton = document.getElementById('pf-zoom-out-button') as HTMLButtonElement |
            null;
        if (zoomOutButton != null) {
            zoomOutButton.addEventListener('click', () => {
                this.view.then(view => view.zoomOut());
            }, false);
        }

        this.filePickerView = FilePickerView.create();
        if (this.filePickerView != null) {
            this.filePickerView.onFileLoaded = fileData => this.fileLoaded(fileData, null);
        }

        const selectFileElement = document.getElementById('pf-select-file') as
            (HTMLSelectElement | null);
        if (selectFileElement != null) {
            selectFileElement.addEventListener('click',
                                               event => this.fileSelectionChanged(event),
                                               false);
        }

        this.fpsLabel = document.getElementById('pf-fps-label');

        const shaderLoader = new ShaderLoader;
        shaderLoader.load();

        this.view = Promise.all([shaderLoader.common, shaderLoader.shaders]).then(allShaders => {
            this.commonShaderSource = allShaders[0];
            this.shaderSources = allShaders[1];
            return this.createView();
        });

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as
            (HTMLSelectElement | null);
        if (this.aaLevelSelect != null)
            this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);

        // The event listeners here use `window.setTimeout()` because jQuery won't fire the "live"
        // click listener that Bootstrap sets up until the event bubbles up to the document. This
        // click listener is what toggles the `checked` attribute, so we have to wait until it
        // fires before updating the antialiasing settings.
        this.subpixelAARadioButton =
            document.getElementById('pf-subpixel-aa-select-on') as HTMLInputElement | null;
        const subpixelAAButtons =
            document.getElementById('pf-subpixel-aa-buttons') as HTMLElement | null;
        if (subpixelAAButtons != null) {
            subpixelAAButtons.addEventListener('click', () => {
                window.setTimeout(() => this.updateAALevel(), 0);
            }, false);
        }

        this.stemDarkeningRadioButton =
            document.getElementById('pf-stem-darkening-select-on') as HTMLInputElement | null;
        const stemDarkeningButtons =
            document.getElementById('pf-stem-darkening-buttons') as HTMLElement | null;
        if (stemDarkeningButtons != null) {
            stemDarkeningButtons.addEventListener('click', () => {
                window.setTimeout(() => this.updateAALevel(), 0);
            }, false);
        }

        this.updateAALevel();
    }

    newTimingsReceived(timings: Partial<Timings>) {
        if (this.fpsLabel == null)
            return;

        while (this.fpsLabel.lastChild != null)
            this.fpsLabel.removeChild(this.fpsLabel.lastChild);

        for (const timing of Object.keys(timings) as Array<keyof Timings>) {
            const tr = document.createElement('div');
            tr.classList.add('row');

            const keyTD = document.createElement('div');
            const valueTD = document.createElement('div');
            keyTD.classList.add('col');
            valueTD.classList.add('col');
            keyTD.appendChild(document.createTextNode(TIMINGS[timing]));
            valueTD.appendChild(document.createTextNode(timings[timing] + " ms"));

            tr.appendChild(keyTD);
            tr.appendChild(valueTD);
            this.fpsLabel.appendChild(tr);
        }

        this.fpsLabel.classList.remove('invisible');
    }

    protected abstract createView(): View;

    private updateAALevel() {
        let aaType: AntialiasingStrategyName, aaLevel: number;
        if (this.aaLevelSelect != null) {
            const selectedOption = this.aaLevelSelect.selectedOptions[0];
            const aaValues = unwrapNull(/^([a-z-]+)(?:-([0-9]+))?$/.exec(selectedOption.value));
            aaType = aaValues[1] as AntialiasingStrategyName;
            aaLevel = aaValues[2] === "" ? 1 : parseInt(aaValues[2], 10);
        } else {
            aaType = 'none';
            aaLevel = 0;
        }

        let subpixelAA: SubpixelAAType;
        if (this.subpixelAARadioButton != null && this.subpixelAARadioButton.checked)
            subpixelAA = 'medium';
        else
            subpixelAA = 'none';

        let stemDarkening: StemDarkeningMode;
        if (this.stemDarkeningRadioButton != null && this.stemDarkeningRadioButton.checked)
            stemDarkening = 'dark';
        else
            stemDarkening = 'none';

        this.view.then(view => {
            view.setAntialiasingOptions(aaType, aaLevel, subpixelAA, stemDarkening);
        });
    }

    private fileSelectionChanged(event: Event) {
        const selectFileElement = event.currentTarget as HTMLSelectElement;
        const selectedOption = selectFileElement.selectedOptions[0] as HTMLOptionElement;

        if (selectedOption.value === 'load-custom' && this.filePickerView != null) {
            this.filePickerView.open();

            const oldSelectedIndex = selectFileElement.selectedIndex;
            const newOption = document.createElement('option');
            newOption.id = 'pf-custom-option-placeholder';
            newOption.appendChild(document.createTextNode("Custom"));
            selectFileElement.insertBefore(newOption, selectedOption);
            selectFileElement.selectedIndex = oldSelectedIndex;
            return;
        }

        // Remove the "Custom…" placeholder if it exists.
        const placeholder = document.getElementById('pf-custom-option-placeholder');
        if (placeholder != null)
            selectFileElement.removeChild(placeholder);

        // Fetch the file.
        this.fetchFile(selectedOption.value, this.builtinFileURI);
    }
}
