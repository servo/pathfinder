// pathfinder/client/src/app-controller.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AntialiasingStrategyName} from "./aa-strategy";
import {ShaderLoader, ShaderMap, ShaderProgramSource} from './shader-loader';
import {expectNotNull, unwrapUndef, unwrapNull} from './utils';
import {PathfinderDemoView} from "./view";

export abstract class AppController {
    start() {
        const canvas = document.getElementById('pf-canvas') as HTMLCanvasElement;
    }

    protected loadInitialFile() {
        const selectFileElement = document.getElementById('pf-select-file') as
            (HTMLSelectElement | null);
        if (selectFileElement != null) {
            const selectedOption = selectFileElement.selectedOptions[0] as HTMLOptionElement;
            this.fetchFile(selectedOption.value);
        } else {
            this.fetchFile(this.defaultFile);
        }
    }

    protected fetchFile(file: string) {
        window.fetch(`${this.builtinFileURI}/${file}`)
              .then(response => response.arrayBuffer())
              .then(data => {
                  this.fileData = data;
                  this.fileLoaded();
              });
    }

    protected canvas: HTMLCanvasElement;

    protected fileData: ArrayBuffer;

    protected abstract fileLoaded(): void;

    protected abstract get defaultFile(): string;
    protected abstract get builtinFileURI(): string;
}

export abstract class DemoAppController<View extends PathfinderDemoView> extends AppController {
    constructor() {
        super();
    }

    start() {
        super.start();

        this.settingsCard = document.getElementById('pf-settings') as HTMLElement;
        this.settingsButton = document.getElementById('pf-settings-button') as HTMLButtonElement;
        this.settingsCloseButton = document.getElementById('pf-settings-close-button') as
            HTMLButtonElement;

        this.settingsButton.addEventListener('click', event => {
            event.stopPropagation();
            this.settingsCard.classList.toggle('pf-invisible');
        }, false);
        this.settingsCloseButton.addEventListener('click', () => {
            this.settingsCard.classList.add('pf-invisible');
        }, false);
        document.body.addEventListener('click', () => {
            this.settingsCard.classList.add('pf-invisible');
        }, false);

        this.filePickerElement = document.getElementById('pf-file-select') as
            (HTMLInputElement | null);
        if (this.filePickerElement != null) {
            this.filePickerElement.addEventListener('change',
                                                    event => this.loadFile(event),
                                                    false);
        }

        const selectFileElement = document.getElementById('pf-select-file') as
            (HTMLSelectElement | null);
        if (selectFileElement != null) {
            selectFileElement.addEventListener('click',
                                               event => this.fileSelectionChanged(event),
                                               false);
        }

        const shaderLoader = new ShaderLoader;
        shaderLoader.load();

        this.view = Promise.all([shaderLoader.common, shaderLoader.shaders]).then(allShaders => {
            this.commonShaderSource = allShaders[0];
            this.shaderSources = allShaders[1];
            return this.createView();
        });

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as HTMLSelectElement;
        this.subpixelAASwitch =
            document.getElementById('pf-subpixel-aa') as HTMLInputElement | null;
        this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);
        if (this.subpixelAASwitch != null)
            this.subpixelAASwitch.addEventListener('change', () => this.updateAALevel(), false);
        this.updateAALevel();
    }

    private updateAALevel() {
        const selectedOption = this.aaLevelSelect.selectedOptions[0];
        const aaValues = unwrapNull(/^([a-z-]+)(?:-([0-9]+))?$/.exec(selectedOption.value));
        const aaType = aaValues[1] as AntialiasingStrategyName;
        const aaLevel = aaValues[2] === "" ? 1 : parseInt(aaValues[2]); 
        const subpixelAA = this.subpixelAASwitch == null ? false : this.subpixelAASwitch.checked;
        this.view.then(view => view.setAntialiasingOptions(aaType, aaLevel, subpixelAA));
    }

    protected loadFile(event: Event) {
        const filePickerElement = event.target as HTMLInputElement;
        const file = expectNotNull(filePickerElement.files, "No file selected!")[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            this.fileData = reader.result;
            this.fileLoaded();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    private fileSelectionChanged(event: Event) {
        const selectFileElement = event.currentTarget as HTMLSelectElement;
        const selectedOption = selectFileElement.selectedOptions[0] as HTMLOptionElement;

        if (selectedOption.value === 'load-custom' && this.filePickerElement != null) {
            this.filePickerElement.click();

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
        this.fetchFile(selectedOption.value);
    }

    protected abstract createView(): View;

    view: Promise<View>;

    protected filePickerElement: HTMLInputElement | null;

    protected commonShaderSource: string | null;
    protected shaderSources: ShaderMap<ShaderProgramSource> | null;

    private aaLevelSelect: HTMLSelectElement;
    private subpixelAASwitch: HTMLInputElement | null;

    private settingsCard: HTMLElement;
    private settingsButton: HTMLButtonElement;
    private settingsCloseButton: HTMLButtonElement;
}
