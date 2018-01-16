// pathfinder/client/src/app-controller.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AntialiasingStrategyName, GammaCorrectionMode, StemDarkeningMode} from "./aa-strategy";
import {SubpixelAAType} from "./aa-strategy";
import {FilePickerView} from "./file-picker";
import {ShaderLoader, ShaderMap, ShaderProgramSource} from './shader-loader';
import {expectNotNull, unwrapNull, unwrapUndef} from './utils';
import {DemoView, Timings, TIMINGS} from "./view";

const GAMMA_LUT_URI: string = "/textures/gamma-lut.png";

const SWITCHES: SwitchMap = {
    gammaCorrection: {
        defaultValue: 'on',
        id: 'pf-gamma-correction',
        offValue: 'off',
        onValue: 'on',
        switchInputsName: 'gammaCorrectionSwitchInputs',
    },
    stemDarkening: {
        defaultValue: 'dark',
        id: 'pf-stem-darkening',
        offValue: 'none',
        onValue: 'dark',
        switchInputsName: 'stemDarkeningSwitchInputs',
    },
    subpixelAA: {
        defaultValue: 'none',
        id: 'pf-subpixel-aa',
        offValue: 'none',
        onValue: 'medium',
        switchInputsName: 'subpixelAASwitchInputs',
    },
};

interface SwitchDescriptor {
    id: string;
    switchInputsName: keyof Switches;
    onValue: string;
    offValue: string;
    defaultValue: string;
}

interface SwitchMap {
    gammaCorrection: SwitchDescriptor;
    stemDarkening: SwitchDescriptor;
    subpixelAA: SwitchDescriptor;
}

export interface AAOptions {
    gammaCorrection: GammaCorrectionMode;
    stemDarkening: StemDarkeningMode;
    subpixelAA: SubpixelAAType;
}

export interface SwitchInputs {
    on: HTMLInputElement;
    off: HTMLInputElement;
}

interface Switches {
    subpixelAASwitchInputs: SwitchInputs | null;
    gammaCorrectionSwitchInputs: SwitchInputs | null;
    stemDarkeningSwitchInputs: SwitchInputs | null;
}

export abstract class AppController {
    protected canvas: HTMLCanvasElement;

    protected selectFileElement: HTMLSelectElement | null;

    protected screenshotButton: HTMLButtonElement | null;

    start(): void {
        this.selectFileElement = document.getElementById('pf-select-file') as HTMLSelectElement |
            null;
    }

    protected loadInitialFile(builtinFileURI: string): void {
        if (this.selectFileElement != null) {
            const selectedOption = this.selectFileElement.selectedOptions[0] as HTMLOptionElement;
            this.fetchFile(selectedOption.value, builtinFileURI);
        } else {
            this.fetchFile(this.defaultFile, builtinFileURI);
        }
    }

    protected fetchFile(file: string, builtinFileURI: string): Promise<void> {
        return new Promise(resolve => {
            window.fetch(`${builtinFileURI}/${file}`)
                  .then(response => response.arrayBuffer())
                  .then(data => {
                      this.fileLoaded(data, file);
                      resolve();
                  });
        });
    }

    protected abstract fileLoaded(data: ArrayBuffer, builtinName: string | null): void;

    protected abstract get defaultFile(): string;
}

export abstract class DemoAppController<View extends DemoView> extends AppController
                                                               implements Switches {
    view: Promise<View>;

    subpixelAASwitchInputs: SwitchInputs | null;
    gammaCorrectionSwitchInputs: SwitchInputs | null;
    stemDarkeningSwitchInputs: SwitchInputs | null;

    protected abstract readonly builtinFileURI: string;

    protected filePickerView: FilePickerView | null;

    protected aaLevelSelect: HTMLSelectElement | null;

    private fpsLabel: HTMLElement | null;

    constructor() {
        super();
    }

    start() {
        super.start();

        this.initPopup('pf-settings', 'pf-settings-button', 'pf-settings-close-button');
        this.initPopup('pf-rotate-slider-card', 'pf-rotate-button', 'pf-rotate-close-button');

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

        const zoomPulseButton = document.getElementById('pf-zoom-pulse-button') as
            HTMLButtonElement | null;
        if (zoomPulseButton != null) {
            zoomPulseButton.addEventListener('click', () => {
                this.view.then(view => view.zoomPulse());
            }, false);
        }

        const rotateSlider = document.getElementById('pf-rotate-slider') as HTMLInputElement |
            null;
        if (rotateSlider != null) {
            rotateSlider.addEventListener('input', event => {
                this.view.then(view => {
                    view.rotate((event.target as HTMLInputElement).valueAsNumber);
                });
            }, false);
        }

        this.filePickerView = FilePickerView.create();
        if (this.filePickerView != null) {
            this.filePickerView.onFileLoaded = fileData => this.fileLoaded(fileData, null);
        }

        if (this.selectFileElement != null) {
            this.selectFileElement
                .addEventListener('click', event => this.fileSelectionChanged(event), false);
        }

        this.fpsLabel = document.getElementById('pf-fps-label');

        const shaderLoader = new ShaderLoader;
        shaderLoader.load();

        const gammaLUTPromise = this.loadGammaLUT();

        const promises: any[] = [gammaLUTPromise, shaderLoader.common, shaderLoader.shaders];
        this.view = Promise.all(promises).then(assets => {
            return this.createView(assets[0], assets[1], assets[2]);
        });

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as
            (HTMLSelectElement | null);
        if (this.aaLevelSelect != null)
            this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);

        // The event listeners here use `window.setTimeout()` because jQuery won't fire the "live"
        // click listener that Bootstrap sets up until the event bubbles up to the document. This
        // click listener is what toggles the `checked` attribute, so we have to wait until it
        // fires before updating the antialiasing settings.
        for (const switchName of Object.keys(SWITCHES) as Array<keyof SwitchMap>) {
            const switchInputsName = SWITCHES[switchName].switchInputsName;
            const switchID = SWITCHES[switchName].id;
            const switchOnInput = document.getElementById(`${switchID}-select-on`);
            const switchOffInput = document.getElementById(`${switchID}-select-off`);
            if (switchOnInput != null && switchOffInput != null) {
                this[switchInputsName] = {
                    off: switchOffInput as HTMLInputElement,
                    on: switchOnInput as HTMLInputElement,
                };
            } else {
                this[switchInputsName] = null;
            }

            const buttons = document.getElementById(`${switchID}-buttons`) as HTMLElement | null;
            if (buttons == null)
                continue;

            buttons.addEventListener('click', () => {
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

    protected updateAALevel(): Promise<void> {
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

        this.updateUIForAALevelChange(aaType, aaLevel);

        const aaOptions: Partial<AAOptions> = {};
        for (const switchName of Object.keys(SWITCHES) as Array<keyof SwitchMap>) {
            const switchDescriptor = SWITCHES[switchName];
            const switchInputsName = switchDescriptor.switchInputsName;
            const switchInputs = this[switchInputsName];
            if (switchInputs == null)
                aaOptions[switchName] = switchDescriptor.defaultValue as any;
            else if (switchInputs.on.checked && !switchInputs.on.disabled)
                aaOptions[switchName] = switchDescriptor.onValue as any;
            else
                aaOptions[switchName] = switchDescriptor.offValue as any;
        }

        return this.view.then(view => {
            view.setAntialiasingOptions(aaType, aaLevel, aaOptions as AAOptions);
        });
    }

    protected updateUIForAALevelChange(aaType: AntialiasingStrategyName, aaLevel: number): void {
        // Overridden by subclasses.
    }

    protected abstract createView(gammaLUT: HTMLImageElement,
                                  commonShaderSource: string,
                                  shaderSources: ShaderMap<ShaderProgramSource>):
                                  View;

    private initPopup(cardID: string, popupButtonID: string, closeButtonID: string): void {
        const card = document.getElementById(cardID) as HTMLElement | null;
        const button = document.getElementById(popupButtonID) as HTMLButtonElement | null;
        const closeButton = document.getElementById(closeButtonID) as HTMLButtonElement | null;

        if (button != null) {
            button.addEventListener('click', event => {
                event.stopPropagation();
                unwrapNull(card).classList.toggle('pf-invisible');
            }, false);
        }
        if (closeButton != null) {
            closeButton.addEventListener('click', () => {
                unwrapNull(card).classList.add('pf-invisible');
            }, false);
        }

        if (card == null)
            return;

        document.body.addEventListener('click', event => {
            let element = event.target as Element | null;
            while (element != null) {
                if (element === card)
                    return;
                element = element.parentElement;
            }

            card.classList.add('pf-invisible');
        }, false);
    }

    private loadGammaLUT(): Promise<HTMLImageElement> {
        return window.fetch(GAMMA_LUT_URI)
                     .then(response => response.blob())
                     .then(blob => {
                         const imgElement = document.createElement('img');
                         imgElement.src = URL.createObjectURL(blob);
                         const promise: Promise<HTMLImageElement> = new Promise(resolve => {
                             imgElement.addEventListener('load', () => {
                                 resolve(imgElement);
                             }, false);
                         });
                         return promise;
                     });
    }

    private fileSelectionChanged(event: Event): void {
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

export function setSwitchInputsValue(switchInputs: SwitchInputs, on: boolean): void {
    switchInputs.on.checked = on;
    switchInputs.off.checked = !on;
}
