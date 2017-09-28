// pathfinder/client/src/file-picker.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {expectNotNull} from "./utils";

export class FilePickerView {
    static create(): FilePickerView | null {
        const element = document.getElementById('pf-file-select') as (HTMLInputElement | null);
        return element == null ? null : new FilePickerView(element);
    }

    onFileLoaded: ((fileData: ArrayBuffer) => void) | null;

    private readonly element: HTMLInputElement;

    private constructor(element: HTMLInputElement) {
        this.element = element;
        this.onFileLoaded = null;
        element.addEventListener('change', event => this.loadFile(event), false);
    }

    open() {
        this.element.click();
    }

    private loadFile(event: Event) {
        const element = event.target as HTMLInputElement;
        const file = expectNotNull(element.files, "No file selected!")[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            if (this.onFileLoaded != null)
                this.onFileLoaded(reader.result);
        }, false);
        reader.readAsArrayBuffer(file);
    }
}
