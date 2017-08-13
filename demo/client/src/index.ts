// pathfinder/demo/src/index.ts

const base64js = require('base64-js');
const opentype = require('opentype.js');

const TEXT: string = "A";
const FONT_SIZE: number = 16.0;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

const COMMON_SHADER_URL: string = '/glsl/gles2/common.inc.glsl';

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
    [shaderName: string]: { 'vertex': string, 'fragment': string };
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
        throw new PathfinderError(message);
    return value;
}

class PathfinderError extends Error {
    constructor(message?: string | undefined) {
        super(message);
    }
}

class PathfinderMeshes {
    constructor(encodedResponse: string) {
        const response = JSON.parse(encodedResponse);
        if (!('Ok' in response))
            throw new PathfinderError("Failed to partition the font!");
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
            throw new PathfinderError("The font type is unsupported.");

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

        this.shaderProgramsPromise = this.loadShaders().then(shaders => this.linkShaders(shaders));

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
        return window.fetch(COMMON_SHADER_URL)
                     .then((response) => response.text())
                     .then((commonSource) => {
            const shaderKeys = Object.keys(SHADER_URLS);

            let promises = [];
            for (const shaderKey of shaderKeys) {
                for (const typeName of ['vertex', 'fragment'] as Array<ShaderTypeName>) {
                    const type = {
                        vertex: this.gl.VERTEX_SHADER,
                        fragment: this.gl.FRAGMENT_SHADER,
                    }[typeName];

                    const url = SHADER_URLS[shaderKey][typeName];
                    promises.push(window.fetch(url)
                                        .then(response => response.text())
                                        .then(source => {
                        const shader = this.gl.createShader(type);
                        if (shader == null)
                            throw new PathfinderError("Failed to create shader!");
                        this.gl.shaderSource(shader, commonSource + "\n#line 1\n" + source);
                        this.gl.compileShader(shader);
                        if (this.gl.getShaderParameter(shader, this.gl.COMPILE_STATUS) == 0) {
                            const infoLog = this.gl.getShaderInfoLog(shader);
                            throw new PathfinderError(`Failed to compile ${typeName} shader ` +
                                                    `"${shaderKey}":\n${infoLog}`);
                        }

                        if (!(shaderKey in shaders))
                            shaders[shaderKey] = {};
                        shaders[shaderKey][type] = shader;
                    }));
                }
            }

            return Promise.all(promises);
        }).then(() => shaders);
    }

    linkShaders(shaders: ShaderMap): Promise<ShaderProgramMap> {
        return new Promise((resolve, reject) => {
            let shaderProgramMap: ShaderProgramMap = {};
            for (const shaderKey of Object.keys(shaders)) {
                const program = expect(this.gl.createProgram(), "Failed to create shader program!");
                const compiledShaders = shaders[shaderKey];
                for (const compiledShader of Object.values(compiledShaders))
                    this.gl.attachShader(program, compiledShader);
                this.gl.linkProgram(program);

                if (this.gl.getProgramParameter(program, this.gl.LINK_STATUS) == 0) {
                    const infoLog = this.gl.getProgramInfoLog(program);
                    throw new PathfinderError(`Failed to link program "${program}":\n${infoLog}`);
                }

                shaderProgramMap[shaderKey] = program;
            }

            resolve(shaderProgramMap);
        });
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
    constructor(vertexShader: WebGLShader, fragmentShader: WebGLShader) {
        // TODO(pcwalton)
    }
}

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
