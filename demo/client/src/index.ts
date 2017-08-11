// pathfinder/demo/src/index.ts

const base64js = require('base64-js');
const opentype = require('opentype.js');

const TEXT: string = "A";
const FONT_SIZE: number = 16.0;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

class PathfinderMeshes {
    constructor(encodedResponse: string) {
        const response = JSON.parse(encodedResponse);
        if (!('Ok' in response))
            throw new Error("Failed to partition the font!");
        const meshes = response.Ok;
        this.bQuadPositions = base64js.toByteArray(meshes.bQuadPositions);
        this.bQuadInfo = base64js.toByteArray(meshes.bQuadInfo);
        this.bVertices = base64js.toByteArray(meshes.bVertices);
    }

    bQuadPositions: ArrayBuffer;
    bQuadInfo: ArrayBuffer;
    bVertices: ArrayBuffer;
}

class AppController {
    constructor() {}

    start() {
        this.view = new PathfinderView(document.getElementById('pf-canvas') as HTMLCanvasElement);

        this.loadFontButton = document.getElementById('pf-load-font-button') as HTMLInputElement;
        this.loadFontButton.addEventListener('change', () => this.loadFont(), false);
    }

    loadFont() {
        const file = this.loadFontButton.files[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            this.fontData = reader.result;
            this.fontLoaded();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    fontLoaded() {
        this.font = opentype.parse(this.fontData);
        if (!this.font.supported) {
            window.alert("The font type is unsupported.");
            return;
        }

        const glyphIDs = this.font.stringToGlyphs(TEXT).map(glyph => glyph.index);

        const request = {
            otf: base64js.fromByteArray(new Uint8Array(this.fontData)),
            fontIndex: 0,
            glyphIDs: glyphIDs,
            pointSize: FONT_SIZE,
        };

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            this.meshes = new PathfinderMeshes(xhr.responseText);
            this.meshesReceived();
        }, false);
        xhr.open('POST', PARTITION_FONT_ENDPOINT_URL, true);
        xhr.setRequestHeader('Content-Type', 'application/json');
        xhr.send(JSON.stringify(request));
    }

    meshesReceived() {
        // TODO(pcwalton)
    }

    view: PathfinderView;
    loadFontButton: HTMLInputElement;
    fontData: ArrayBuffer;
    font: any;
    meshes: PathfinderMeshes;
}

class PathfinderView {
    constructor(canvas: HTMLCanvasElement) {
        this.canvas = canvas;
    }

    canvas: HTMLCanvasElement;
}

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
