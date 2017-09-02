// pathfinder/client/src/mesh-debugger.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AppController} from "./app-controller";
import {BUILTIN_FONT_URI} from "./text";
import {unwrapNull} from "./utils";

const DEFAULT_FONT: string = 'nimbus-sans';

class MeshDebuggerAppController extends AppController {
    start() {
        super.start();

        this.loadInitialFile();
 
    }
    protected fileLoaded(): void {
        throw new Error("Method not implemented.");
    }

    protected get defaultFile(): string {
        return DEFAULT_FONT;
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }
}

function main() {
    const appController = new MeshDebuggerAppController;
    appController.start();
}

main();
