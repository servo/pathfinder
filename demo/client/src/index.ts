// pathfinder/demo/src/index.ts

const base64js = require('base64-js');
const opentype = require('opentype.js');

const TEXT: string = "A";
const FONT_SIZE: number = 16.0;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

const SHADER_URLS: ShaderURLMap = {
    directCurve: {
        vertex: "/glsl/gles2/direct-curve.vs.glsl",
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
    },
    directInterior: {
        vertex: "/glsl/gles2/direct-interior.vs.glsl",
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
    },
};

interface ShaderURLMap {
    [shaderName: string]: { 'vertex': string, 'fragment': string }
}

interface ShaderMap {
    [shaderName: string]: { [shaderType: number]: WebGLShader };
}

interface ShaderProgramMap {
    [shaderProgramName: string]: PathfinderShaderProgram;
}

type ShaderType = number;

type ShaderTypeName = 'vertex' | 'fragment';

function expect<T>(value: T | null, message: string): T {
    if (value == null)
        throw new Error(message);
    return value;
}

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
        const file = expect(this.loadFontButton.files, "No file selected!")[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            this.fontData = reader.result;
            this.fontLoaded();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    fontLoaded() {
        this.font = opentype.parse(this.fontData);
        if (!this.font.supported)
            throw new Error("The font type is unsupported.");

        const glyphIDs = this.font.stringToGlyphs(TEXT).map((glyph: any) => glyph.index);

        const request = {
            otf: base64js.fromByteArray(new Uint8Array(this.fontData)),
            fontIndex: 0,
            glyphIDs: glyphIDs,
            pointSize: FONT_SIZE,
        };

        window.fetch(PARTITION_FONT_ENDPOINT_URL, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(request),
        }).then((response) => {
            response.text().then((encodedMeshes) => {
                this.meshes = new PathfinderMeshes(encodedMeshes);
                this.meshesReceived();
            });
        });
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

        this.initContext();

        this.loadShaders().then(shaders => this.shaderProgramsPromise = this.linkShaders(shaders));

        window.addEventListener('resize', () => this.resizeToFit(), false);
        this.resizeToFit();
    }

    initContext() {
        this.gl = expect(this.canvas.getContext('webgl', { antialias: false, depth: true }),
                         "Failed to initialize WebGL! Check that your browser supports it.");

        this.gl.clearColor(0.0, 0.0, 1.0, 1.0);
        this.gl.clear(this.gl.COLOR_BUFFER_BIT);
        this.gl.flush();
    }

    loadShaders(): Promise<ShaderMap> {
        let shaders: ShaderMap = {};
        const shaderKeys = Object.keys(SHADER_URLS);

        let promises = [];
        for (const shaderKey of shaderKeys) {
            for (const typeName of ['vertex', 'fragment'] as Array<ShaderTypeName>) {
                const type = {
                    vertex: this.gl.VERTEX_SHADER,
                    fragment: this.gl.FRAGMENT_SHADER,
                }[typeName];

                const url = SHADER_URLS[shaderKey][typeName];
                promises.push(window.fetch(url).then((response) => {
                    return response.text().then((source) => {
                        const shader = this.gl.createShader(type);
                        if (shader == null)
                            throw new Error("Failed to create shader!");
                        this.gl.shaderSource(shader, source);
                        this.gl.compileShader(shader);
                        if (!(shaderKey in shaders))
                            shaders[shaderKey] = {};
                        shaders[shaderKey][type] = shader;
                    });
                }));
            }
        }

        return Promise.all(promises).then(() => shaders);
    }

    linkShaders(shaders: ShaderMap): Promise<ShaderProgramMap> {
        // TODO(pcwalton)
        throw new Error("TODO");
    }

    resizeToFit() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight - this.canvas.scrollTop;
    }

    canvas: HTMLCanvasElement;
    gl: WebGLRenderingContext;
    shaderProgramsPromise: Promise<ShaderProgramMap>;
}

class PathfinderShaderProgram {
    constructor(vertexShaderSource: string, fragmentShaderSource: string) {
        // TODO(pcwalton)
    }
}

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
