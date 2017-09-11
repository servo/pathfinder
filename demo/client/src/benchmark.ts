// pathfinder/client/src/benchmark.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as opentype from "opentype.js";

import {AppController} from "./app-controller";
import {PathfinderMeshData} from "./meshes";
import {BUILTIN_FONT_URI, GlyphStorage, PathfinderGlyph, TextFrame, TextRun} from "./text";
import {assert, unwrapNull} from "./utils";

const STRING: string = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

const FONT: string = 'nimbus-sans';

class BenchmarkAppController extends AppController {
    start() {
        super.start();

        const runBenchmarkButton = unwrapNull(document.getElementById('pf-run-benchmark-button'));
        runBenchmarkButton.addEventListener('click', () => this.runBenchmark(), false);

        this.loadInitialFile();
    }

    protected fileLoaded(): void {
        const font = opentype.parse(this.fileData);
        assert(font.isSupported(), "The font type is unsupported!");

        const createGlyph = (glyph: opentype.Glyph) => new BenchmarkGlyph(glyph);
        const textRun = new TextRun<BenchmarkGlyph>(STRING, [0, 0], font, createGlyph);
        const textFrame = new TextFrame([textRun], font);
        this.glyphStorage = new GlyphStorage(this.fileData, [textFrame], createGlyph, font);

        this.glyphStorage.partition().then(meshes => {
            this.meshes = meshes;
            // TODO(pcwalton)
            // this.renderer.attachMeshes();
        })
    }

    private runBenchmark(): void {
        // TODO(pcwalton)
    }

    protected readonly defaultFile: string = FONT;
    protected readonly builtinFileURI: string = BUILTIN_FONT_URI;

    private glyphStorage: GlyphStorage<BenchmarkGlyph>;
    private meshes: PathfinderMeshData;
}

class BenchmarkGlyph extends PathfinderGlyph {}
