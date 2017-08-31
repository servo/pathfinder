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
import {PathfinderView} from "./view";

export default abstract class AppController<View extends PathfinderView> {
    constructor() {}

    start() {
        const canvas = document.getElementById('pf-canvas') as HTMLCanvasElement;

        this.settingsCard = document.getElementById('pf-settings') as HTMLElement;
        this.settingsButton = document.getElementById('pf-settings-button') as HTMLButtonElement;
        this.settingsCloseButton = document.getElementById('pf-settings-close-button') as
            HTMLButtonElement;
        this.settingsButton.addEventListener('click', () => {
            this.settingsCard.classList.toggle('pf-invisible');
        }, false);
        this.settingsCloseButton.addEventListener('click', () => {
            this.settingsCard.classList.add('pf-invisible');
        }, false);

        this.filePickerElement = document.getElementById('pf-file-select') as HTMLInputElement;
        this.filePickerElement.addEventListener('change', () => this.loadFile(), false);

        this.selectFileElement = document.getElementById('pf-select-file') as HTMLSelectElement;
        this.selectFileElement.addEventListener('click', () => {
            this.fileSelectionChanged();
        }, false);

        const shaderLoader = new ShaderLoader;
        shaderLoader.load();

        this.view = Promise.all([shaderLoader.common, shaderLoader.shaders]).then(allShaders => {
            return this.createView(canvas, allShaders[0], allShaders[1]);
        });

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as HTMLSelectElement;
        this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);
        this.updateAALevel();
    }

    protected loadInitialFile() {
        this.fileSelectionChanged();
    }

    private updateAALevel() {
        const selectedOption = this.aaLevelSelect.selectedOptions[0];
        const aaValues = unwrapNull(/^([a-z-]+)(?:-([0-9]+))?$/.exec(selectedOption.value));
        const aaType = aaValues[1] as AntialiasingStrategyName;
        const aaLevel = aaValues[2] === "" ? 1 : parseInt(aaValues[2]); 
        this.view.then(view => view.setAntialiasingOptions(aaType, aaLevel));
    }

    protected loadFile() {
        const file = expectNotNull(this.filePickerElement.files, "No file selected!")[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            this.fileData = reader.result;
            this.fileLoaded();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    protected fileSelectionChanged() {
        const selectedOption = this.selectFileElement.selectedOptions[0] as HTMLOptionElement;

        if (selectedOption.value === 'load-custom') {
            this.filePickerElement.click();

            const oldSelectedIndex = this.selectFileElement.selectedIndex;
            const newOption = document.createElement('option');
            newOption.id = 'pf-custom-option-placeholder';
            newOption.appendChild(document.createTextNode("Custom"));
            this.selectFileElement.insertBefore(newOption, selectedOption);
            this.selectFileElement.selectedIndex = oldSelectedIndex;
            return;
        }

        // Remove the "Custom…" placeholder if it exists.
        const placeholder = document.getElementById('pf-custom-option-placeholder');
        if (placeholder != null)
            this.selectFileElement.removeChild(placeholder);

        // Fetch the file.
        window.fetch(`${this.builtinFileURI}/${selectedOption.value}`)
              .then(response => response.arrayBuffer())
              .then(data => {
                  this.fileData = data;
                  this.fileLoaded();
              });
    }

    protected abstract fileLoaded(): void;

    protected abstract get builtinFileURI(): string;

    protected abstract createView(canvas: HTMLCanvasElement,
                                  commonShaderSource: string,
                                  shaderSources: ShaderMap<ShaderProgramSource>): View;

    view: Promise<View>;

    protected fileData: ArrayBuffer;

    protected canvas: HTMLCanvasElement;
    protected selectFileElement: HTMLSelectElement;
    protected filePickerElement: HTMLInputElement;
    private aaLevelSelect: HTMLSelectElement;
    private settingsCard: HTMLElement;
    private settingsButton: HTMLButtonElement;
    private settingsCloseButton: HTMLButtonElement;
}
