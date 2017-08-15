// pathfinder/demo/src/index.ts

const base64js = require('base64-js');
const opentype = require('opentype.js');

const TEXT: string = "G";
const FONT_SIZE: number = 16.0;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

const COMMON_SHADER_URL: string = '/glsl/gles2/common.inc.glsl';

const UINT32_SIZE: number = 4;

const B_POSITION_SIZE: number = 8;

const B_VERTEX_QUAD_SIZE: number = 8;
const B_VERTEX_QUAD_PATH_ID_OFFSET: number = 0;
const B_VERTEX_QUAD_TEX_COORD_OFFSET: number = 4;
const B_VERTEX_QUAD_SIGN_OFFSET: number = 6;

const IDENTITY: Matrix4D = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

const SHADER_URLS: ShaderMap<ShaderProgramURLs> = {
    directCurve: {
        vertex: "/glsl/gles2/direct-curve.vs.glsl",
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
    },
    directInterior: {
        vertex: "/glsl/gles2/direct-interior.vs.glsl",
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
    },
};

interface UnlinkedShaderProgram {
    vertex: WebGLShader;
    fragment: WebGLShader;
}

type Matrix4D = number[];

interface ShaderProgramSource {
    vertex: string;
    fragment: string;
}

interface ShaderProgramURLs {
    vertex: string;
    fragment: string;
}

interface ShaderMap<T> {
    directCurve: T;
    directInterior: T;
}

interface UniformMap {
    [uniformName: string]: WebGLUniformLocation;
}

interface AttributeMap {
    [attributeName: string]: number;
}

type ShaderType = number;

type ShaderTypeName = 'vertex' | 'fragment';

function expect<T>(value: T | null, message: string): T {
    if (value == null)
        throw new PathfinderError(message);
    return value;
}

function unwrap<T>(value: T | null): T {
    return expect(value, "Unexpected null!");
}

class PathfinderError extends Error {
    constructor(message?: string | undefined) {
        super(message);
    }
}

interface Meshes<T> {
    readonly bQuads: T;
    readonly bVertexPositions: T;
    readonly bVertexInfo: T;
    readonly coverInteriorIndices: T;
    readonly coverCurveIndices: T;
    readonly edgeUpperLineIndices: T;
    readonly edgeUpperCurveIndices: T;
    readonly edgeLowerLineIndices: T;
    readonly edgeLowerCurveIndices: T;
}

type BufferType = number;

const BUFFER_TYPES: Meshes<BufferType> = {
    bQuads: WebGLRenderingContext.ARRAY_BUFFER,
    bVertexPositions: WebGLRenderingContext.ARRAY_BUFFER,
    bVertexInfo: WebGLRenderingContext.ARRAY_BUFFER,
    coverInteriorIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
    coverCurveIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
    edgeUpperLineIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
    edgeUpperCurveIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
    edgeLowerLineIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
    edgeLowerCurveIndices: WebGLRenderingContext.ELEMENT_ARRAY_BUFFER,
};

class PathfinderMeshData implements Meshes<ArrayBuffer> {
    constructor(encodedResponse: string) {
        const response = JSON.parse(encodedResponse);
        if (!('Ok' in response))
            throw new PathfinderError("Failed to partition the font!");
        const meshes = response.Ok;
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof PathfinderMeshData>)
            this[bufferName] = base64js.toByteArray(meshes[bufferName]).buffer;
    }

    readonly bQuads: ArrayBuffer;
    readonly bVertexPositions: ArrayBuffer;
    readonly bVertexInfo: ArrayBuffer;
    readonly coverInteriorIndices: ArrayBuffer;
    readonly coverCurveIndices: ArrayBuffer;
    readonly edgeUpperLineIndices: ArrayBuffer;
    readonly edgeUpperCurveIndices: ArrayBuffer;
    readonly edgeLowerLineIndices: ArrayBuffer;
    readonly edgeLowerCurveIndices: ArrayBuffer;
}

class PathfinderMeshBuffers implements Meshes<WebGLBuffer> {
    constructor(gl: WebGLRenderingContext, meshData: PathfinderMeshData) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof PathfinderMeshBuffers>) {
            const bufferType = BUFFER_TYPES[bufferName];
            const buffer = expect(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
            console.log(`${bufferName} has size ${meshData[bufferName].byteLength}`);
            if (bufferName == 'coverInteriorIndices') {
                const typedArray = new Uint32Array(meshData[bufferName]);
                let array = [];
                for (let i = 0; i < typedArray.length; i++)
                    array[i] = typedArray[i];
                console.log(array.toString());
            }
            this[bufferName] = buffer;
        }
    }

    readonly bQuads: WebGLBuffer;
    readonly bVertexPositions: WebGLBuffer;
    readonly bVertexInfo: WebGLBuffer;
    readonly coverInteriorIndices: WebGLBuffer;
    readonly coverCurveIndices: WebGLBuffer;
    readonly edgeUpperLineIndices: WebGLBuffer;
    readonly edgeUpperCurveIndices: WebGLBuffer;
    readonly edgeLowerLineIndices: WebGLBuffer;
    readonly edgeLowerCurveIndices: WebGLBuffer;
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
                this.meshes = new PathfinderMeshData(encodedMeshes);
                this.meshesReceived();
            });
        });
    }

    meshesReceived() {
        this.view.attachMeshes(this.meshes);
    }

    view: PathfinderView;
    loadFontButton: HTMLInputElement;
    fontData: ArrayBuffer;
    font: any;
    meshes: PathfinderMeshData;
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
        this.gl.getExtension('OES_element_index_uint');
    }

    loadShaders(): Promise<ShaderMap<UnlinkedShaderProgram>> {
        let shaders: Partial<ShaderMap<Partial<UnlinkedShaderProgram>>> = {};
        return window.fetch(COMMON_SHADER_URL)
                     .then((response) => response.text())
                     .then((commonSource) => {
            const shaderKeys = Object.keys(SHADER_URLS) as Array<keyof ShaderMap<string>>;

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

                        if (shaders[shaderKey] == null)
                            shaders[shaderKey] = {};
                        shaders[shaderKey]![typeName] = shader;
                    }));
                }
            }

            return Promise.all(promises);
        }).then(() => shaders as ShaderMap<UnlinkedShaderProgram>);
    }

    linkShaders(shaders: ShaderMap<UnlinkedShaderProgram>):
                Promise<ShaderMap<PathfinderShaderProgram>> {
        return new Promise((resolve, reject) => {
            let shaderProgramMap: Partial<ShaderMap<PathfinderShaderProgram>> = {};
            for (const shaderName of Object.keys(shaders) as
                 Array<keyof ShaderMap<UnlinkedShaderProgram>>) {
                shaderProgramMap[shaderName] = new PathfinderShaderProgram(this.gl,
                                                                           shaderName,
                                                                           shaders[shaderName]);
            }

            resolve(shaderProgramMap as ShaderMap<PathfinderShaderProgram>);
        });
    }

    attachMeshes(meshes: PathfinderMeshData) {
        this.meshes = new PathfinderMeshBuffers(this.gl, meshes);
        this.setDirty();
    }

    setDirty() {
        if (this.dirty)
            return;
        this.dirty = true;
        window.requestAnimationFrame(() => this.redraw());
    }

    resizeToFit() {
        const width = window.innerWidth;
        const height = window.scrollY + window.innerHeight -
            this.canvas.getBoundingClientRect().top;
        const devicePixelRatio = window.devicePixelRatio;
        this.canvas.style.width = width + 'px';
        this.canvas.style.height = height + 'px';
        this.canvas.width = width * devicePixelRatio;
        this.canvas.height = height * devicePixelRatio;
        this.setDirty();
    }

    redraw() {
        this.shaderProgramsPromise.then((shaderPrograms: ShaderMap<PathfinderShaderProgram>) => {
            if (this.meshes == null) {
                this.dirty = false;
                return;
            }

            // Clear.
            this.gl.clearColor(1.0, 1.0, 1.0, 1.0);
            this.gl.clearDepth(0.0);
            this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);

            // Set up the depth buffer.
            this.gl.depthFunc(this.gl.GREATER);
            this.gl.depthMask(true);
            this.gl.enable(this.gl.DEPTH_TEST);

            // Set up the implicit cover interior VAO.
            const directInteriorProgram = shaderPrograms.directInterior;
            this.gl.useProgram(directInteriorProgram.program);
            this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
            this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPosition,
                                        2,
                                        this.gl.FLOAT,
                                        false,
                                        0,
                                        0);
            this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexInfo);
            this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPathDepth,
                                        1,
                                        this.gl.UNSIGNED_SHORT, // FIXME(pcwalton)
                                        true,
                                        B_VERTEX_QUAD_SIZE,
                                        B_VERTEX_QUAD_PATH_ID_OFFSET);
            this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
            this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathDepth);
            this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverInteriorIndices);

            // Draw direct interior parts.
            this.gl.uniformMatrix4fv(directInteriorProgram.uniforms.uTransform, false, IDENTITY);
            this.gl.uniform2i(directInteriorProgram.uniforms.uFramebufferSize,
                              this.canvas.width,
                              this.canvas.height);
            let indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                        this.gl.BUFFER_SIZE) / UINT32_SIZE;
            this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);

            // Disable depth writing.
            this.gl.depthMask(false);

            // Set up the implicit cover curve VAO.
            const directCurveProgram = shaderPrograms.directCurve;
            this.gl.useProgram(directCurveProgram.program);
            this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
            this.gl.vertexAttribPointer(directCurveProgram.attributes.aPosition,
                                        2,
                                        this.gl.FLOAT,
                                        false,
                                        0,
                                        0);
            this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexInfo);
            this.gl.vertexAttribPointer(directCurveProgram.attributes.aTexCoord,
                                        2,
                                        this.gl.UNSIGNED_BYTE,
                                        false,
                                        B_VERTEX_QUAD_SIZE,
                                        B_VERTEX_QUAD_TEX_COORD_OFFSET);
            this.gl.vertexAttribPointer(directCurveProgram.attributes.aPathDepth,
                                        1,
                                        this.gl.UNSIGNED_SHORT, // FIXME(pcwalton)
                                        true,
                                        B_VERTEX_QUAD_SIZE,
                                        B_VERTEX_QUAD_PATH_ID_OFFSET);
            this.gl.vertexAttribPointer(directCurveProgram.attributes.aSign,
                                        1,
                                        this.gl.BYTE,
                                        false,
                                        B_VERTEX_QUAD_SIZE,
                                        B_VERTEX_QUAD_SIGN_OFFSET);
            this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
            this.gl.enableVertexAttribArray(directCurveProgram.attributes.aTexCoord);
            this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPathDepth);
            this.gl.enableVertexAttribArray(directCurveProgram.attributes.aSign);
            this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverCurveIndices);

            // Draw direct curve parts.
            this.gl.uniformMatrix4fv(directCurveProgram.uniforms.uTransform, false, IDENTITY);
            this.gl.uniform2i(directCurveProgram.uniforms.uFramebufferSize,
                              this.canvas.width,
                              this.canvas.height);
            indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                    this.gl.BUFFER_SIZE) / UINT32_SIZE;
            this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);

            // Clear dirty bit and finish.
            this.dirty = false;
        });
    }

    canvas: HTMLCanvasElement;
    gl: WebGLRenderingContext;
    shaderProgramsPromise: Promise<ShaderMap<PathfinderShaderProgram>>;
    meshes: PathfinderMeshBuffers;
    dirty: boolean;
}

class PathfinderShaderProgram {
    constructor(gl: WebGLRenderingContext,
                programName: string,
                unlinkedShaderProgram: UnlinkedShaderProgram) {
        this.program = expect(gl.createProgram(), "Failed to create shader program!");
        for (const compiledShader of Object.values(unlinkedShaderProgram))
            gl.attachShader(this.program, compiledShader);
        gl.linkProgram(this.program);

        if (gl.getProgramParameter(this.program, gl.LINK_STATUS) == 0) {
            const infoLog = gl.getProgramInfoLog(this.program);
            throw new PathfinderError(`Failed to link program "${programName}":\n${infoLog}`);
        }

        const uniformCount = gl.getProgramParameter(this.program, gl.ACTIVE_UNIFORMS);
        const attributeCount = gl.getProgramParameter(this.program, gl.ACTIVE_ATTRIBUTES);

        let uniforms: UniformMap = {};
        let attributes: AttributeMap = {};

        for (let uniformIndex = 0; uniformIndex < uniformCount; uniformIndex++) {
            const uniformName = unwrap(gl.getActiveUniform(this.program, uniformIndex)).name;
            uniforms[uniformName] = expect(gl.getUniformLocation(this.program, uniformName),
                                           `Didn't find uniform "${uniformName}"!`);
        }
        for (let attributeIndex = 0; attributeIndex < attributeCount; attributeIndex++) {
            const attributeName = unwrap(gl.getActiveAttrib(this.program, attributeIndex)).name;
            attributes[attributeName] = attributeIndex;
        }

        this.uniforms = uniforms;
        this.attributes = attributes;
    }

    readonly uniforms: UniformMap;
    readonly attributes: AttributeMap;
    readonly program: WebGLProgram;
}

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
