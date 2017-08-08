// pathfinder/demo/src/index.ts

const opentype = require('opentype.js');

class AppController {
    constructor() {
    }

    start() {
        this.view = new PathfinderView(document.getElementById('pf-canvas') as HTMLCanvasElement);

        this.loadFontButton = document.getElementById('pf-load-font-button') as HTMLInputElement;
        this.loadFontButton.addEventListener('change', () => this.loadFont(), false);
    }

    loadFont() {
        const file = this.loadFontButton.files[0];
        const fileURL = window.URL.createObjectURL(file);
        opentype.load(fileURL, (err, font) => this.fontLoaded(font));
    }

    fontLoaded(font) {
        // TODO(pcwalton)
    }

    view: PathfinderView;
    loadFontButton: HTMLInputElement;
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
