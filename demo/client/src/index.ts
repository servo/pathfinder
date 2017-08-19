// pathfinder/demo/src/index.ts
//
// Copyright © 2017 Mozilla Foundation

const base64js = require('base64-js');
const glmatrix = require('gl-matrix');
const opentype = require('opentype.js');

const TEXT: string = "G";
const FONT_SIZE: number = 16.0;

const SCALE_FACTOR: number = 1.0 / 100.0;

const TIME_INTERVAL_DELAY: number = 32;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

const COMMON_SHADER_URL: string = '/glsl/gles2/common.inc.glsl';

const UINT32_SIZE: number = 4;

const B_POSITION_SIZE: number = 8;

const B_PATH_INDEX_SIZE: number = 2;

const B_LOOP_BLINN_DATA_SIZE: number = 4;
const B_LOOP_BLINN_DATA_TEX_COORD_OFFSET: number = 0;
const B_LOOP_BLINN_DATA_SIGN_OFFSET: number = 2;

const B_QUAD_SIZE: number = 4 * 8;
const B_QUAD_UPPER_INDICES_OFFSET: number = 0;
const B_QUAD_LOWER_INDICES_OFFSET: number = 4 * 4;

const SHADER_URLS: ShaderMap<ShaderProgramURLs> = {
    blit: {
        vertex: "/glsl/gles2/blit.vs.glsl",
        fragment: "/glsl/gles2/blit.fs.glsl",
    },
    directCurve: {
        vertex: "/glsl/gles2/direct-curve.vs.glsl",
        fragment: "/glsl/gles2/direct-curve.fs.glsl",
    },
    directInterior: {
        vertex: "/glsl/gles2/direct-interior.vs.glsl",
        fragment: "/glsl/gles2/direct-interior.fs.glsl",
    },
    ecaaEdgeDetect: {
        vertex: "/glsl/gles2/ecaa-edge-detect.vs.glsl",
        fragment: "/glsl/gles2/ecaa-edge-detect.fs.glsl",
    },
    ecaaCover: {
        vertex: "/glsl/gles2/ecaa-cover.vs.glsl",
        fragment: "/glsl/gles2/ecaa-cover.fs.glsl",
    },
    ecaaLine: {
        vertex: "/glsl/gles2/ecaa-line.vs.glsl",
        fragment: "/glsl/gles2/ecaa-line.fs.glsl",
    },
    ecaaCurve: {
        vertex: "/glsl/gles2/ecaa-curve.vs.glsl",
        fragment: "/glsl/gles2/ecaa-curve.fs.glsl",
    },
    ecaaResolve: {
        vertex: "/glsl/gles2/ecaa-resolve.vs.glsl",
        fragment: "/glsl/gles2/ecaa-resolve.fs.glsl",
    },
};

interface UnlinkedShaderProgram {
    vertex: WebGLShader;
    fragment: WebGLShader;
}

type Matrix4D = Float32Array;

interface Point2D {
    x: number;
    y: number;
}

interface Size2D {
    width: number;
    height: number;
}

interface ShaderProgramSource {
    vertex: string;
    fragment: string;
}

interface ShaderProgramURLs {
    vertex: string;
    fragment: string;
}

interface ShaderMap<T> {
    blit: T;
    directCurve: T;
    directInterior: T;
    ecaaEdgeDetect: T;
    ecaaCover: T;
    ecaaLine: T;
    ecaaCurve: T;
    ecaaResolve: T;
}

interface UniformMap {
    [uniformName: string]: WebGLUniformLocation;
}

interface AttributeMap {
    [attributeName: string]: number;
}

interface UpperAndLower<T> {
    upper: T;
    lower: T;
}

interface AntialiasingStrategy {
    // Prepares any OpenGL data. This is only called on startup and canvas resize.
    init(view: PathfinderView, framebufferSize: Size2D): void;

    // Uploads any mesh data. This is called whenever a new set of meshes is supplied.
    attachMeshes(view: PathfinderView): void;

    // Called before direct rendering.
    //
    // Typically, this redirects direct rendering to a framebuffer of some sort.
    prepare(view: PathfinderView): void;

    // Called after direct rendering.
    //
    // This usually performs the actual antialiasing and blits to the real framebuffer.
    resolve(view: PathfinderView): void;
}

type ShaderType = number;

type ShaderTypeName = 'vertex' | 'fragment';

type WebGLQuery = any;

type WebGLVertexArrayObject = any;

const QUAD_POSITIONS: Float32Array = new Float32Array([
    -1.0,  1.0,
     1.0,  1.0,
    -1.0, -1.0,
     1.0, -1.0,
]);

const QUAD_TEX_COORDS: Float32Array = new Float32Array([
    0.0, 1.0,
    1.0, 1.0,
    0.0, 0.0,
    1.0, 0.0,
]);

const QUAD_ELEMENTS: Uint8Array = new Uint8Array([2, 0, 1, 1, 3, 2]);

// Various utility functions

function assert(value: boolean, message: string) {
    if (!value)
        throw new PathfinderError(message);
}

function expectNotNull<T>(value: T | null, message: string): T {
    if (value === null)
        throw new PathfinderError(message);
    return value;
}

function expectNotUndef<T>(value: T | undefined, message: string): T {
    if (value === undefined)
        throw new PathfinderError(message);
    return value;
}

function unwrapNull<T>(value: T | null): T {
    return expectNotNull(value, "Unexpected null!");
}

function unwrapUndef<T>(value: T | undefined): T {
    return expectNotUndef(value, "Unexpected `undefined`!");
}

class PathfinderError extends Error {
    constructor(message?: string | undefined) {
        super(message);
    }
}

// GL utilities

function createFramebufferColorTexture(gl: WebGLRenderingContext, size: Size2D): WebGLTexture {
    // Firefox seems to have a bug whereby textures don't get marked as initialized when cleared
    // if they're anything other than the first attachment of an FBO. To work around this, supply
    // zero data explicitly when initializing the texture.

    const texture = unwrapNull(gl.createTexture());
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D,
                  0,
                  gl.RGBA,
                  size.width,
                  size.height,
                  0,
                  gl.RGBA,
                  gl.UNSIGNED_BYTE,
                  new Uint8Array(size.width * size.height * 4));
    setTextureParameters(gl, gl.NEAREST);
    return texture;
}

function createFramebufferDepthTexture(gl: WebGLRenderingContext, size: Size2D): WebGLTexture {
    const texture = unwrapNull(gl.createTexture());
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D,
                  0,
                  gl.DEPTH_COMPONENT,
                  size.width,
                  size.height,
                  0,
                  gl.DEPTH_COMPONENT,
                  gl.UNSIGNED_INT,
                  null);
    setTextureParameters(gl, gl.NEAREST);
    return texture;
}

function setTextureParameters(gl: WebGLRenderingContext, filter: number) {
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
}

function createFramebuffer(gl: WebGLRenderingContext,
                           drawBuffersExt: any,
                           colorAttachments: WebGLTexture[],
                           depthAttachment: WebGLTexture | null):
                           WebGLFramebuffer {
    const framebuffer = unwrapNull(gl.createFramebuffer());
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);

    const colorAttachmentCount = colorAttachments.length;
    for (let colorAttachmentIndex = 0;
         colorAttachmentIndex < colorAttachmentCount;
         colorAttachmentIndex++) {
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                drawBuffersExt[`COLOR_ATTACHMENT${colorAttachmentIndex}_WEBGL`],
                                gl.TEXTURE_2D,
                                colorAttachments[colorAttachmentIndex],
                                0);
    }

    if (depthAttachment != null) {
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                gl.DEPTH_ATTACHMENT,
                                gl.TEXTURE_2D,
                                depthAttachment,
                                0);
    }

    assert(gl.checkFramebufferStatus(gl.FRAMEBUFFER) == gl.FRAMEBUFFER_COMPLETE,
           "Framebuffer was incomplete!");
    return framebuffer;
}

function initQuadVAO(view: PathfinderView, attributes: any) {
    view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
    view.gl.vertexAttribPointer(attributes.aPosition, 2, view.gl.FLOAT, false, 0, 0);
    view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadTexCoordsBuffer);
    view.gl.vertexAttribPointer(attributes.aTexCoord, 2, view.gl.FLOAT, false, 0, 0);
    view.gl.enableVertexAttribArray(attributes.aPosition);
    view.gl.enableVertexAttribArray(attributes.aTexCoord);
    view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
}

interface Meshes<T> {
    readonly bQuads: T;
    readonly bVertexPositions: T;
    readonly bVertexPathIDs: T;
    readonly bVertexLoopBlinnData: T;
    readonly coverInteriorIndices: T;
    readonly coverCurveIndices: T;
    readonly edgeUpperLineIndices: T;
    readonly edgeLowerLineIndices: T;
    readonly edgeUpperCurveIndices: T;
    readonly edgeLowerCurveIndices: T;
}

type BufferType = 'ARRAY_BUFFER' | 'ELEMENT_ARRAY_BUFFER';

const BUFFER_TYPES: Meshes<BufferType> = {
    bQuads: 'ARRAY_BUFFER',
    bVertexPositions: 'ARRAY_BUFFER',
    bVertexPathIDs: 'ARRAY_BUFFER',
    bVertexLoopBlinnData: 'ARRAY_BUFFER',
    coverInteriorIndices: 'ELEMENT_ARRAY_BUFFER',
    coverCurveIndices: 'ELEMENT_ARRAY_BUFFER',
    edgeUpperLineIndices: 'ARRAY_BUFFER',
    edgeLowerLineIndices: 'ARRAY_BUFFER',
    edgeUpperCurveIndices: 'ARRAY_BUFFER',
    edgeLowerCurveIndices: 'ARRAY_BUFFER',
};

class PathfinderMeshData implements Meshes<ArrayBuffer> {
    constructor(encodedResponse: string) {
        const response = JSON.parse(encodedResponse);
        if (!('Ok' in response))
            throw new PathfinderError("Failed to partition the font!");
        const meshes = response.Ok;
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof Meshes<void>>)
            this[bufferName] = base64js.toByteArray(meshes[bufferName]).buffer;

        this.bQuadCount = this.bQuads.byteLength / B_QUAD_SIZE;
        this.edgeUpperLineIndexCount = this.edgeUpperLineIndices.byteLength / 8;
        this.edgeLowerLineIndexCount = this.edgeLowerLineIndices.byteLength / 8;
        this.edgeUpperCurveIndexCount = this.edgeUpperCurveIndices.byteLength / 16;
        this.edgeLowerCurveIndexCount = this.edgeLowerCurveIndices.byteLength / 16;
    }

    readonly bQuads: ArrayBuffer;
    readonly bVertexPositions: ArrayBuffer;
    readonly bVertexPathIDs: ArrayBuffer;
    readonly bVertexLoopBlinnData: ArrayBuffer;
    readonly coverInteriorIndices: ArrayBuffer;
    readonly coverCurveIndices: ArrayBuffer;
    readonly edgeUpperLineIndices: ArrayBuffer;
    readonly edgeLowerLineIndices: ArrayBuffer;
    readonly edgeUpperCurveIndices: ArrayBuffer;
    readonly edgeLowerCurveIndices: ArrayBuffer;

    readonly bQuadCount: number;
    readonly edgeUpperLineIndexCount: number;
    readonly edgeLowerLineIndexCount: number;
    readonly edgeUpperCurveIndexCount: number;
    readonly edgeLowerCurveIndexCount: number;
}

class PathfinderMeshBuffers implements Meshes<WebGLBuffer> {
    constructor(gl: WebGLRenderingContext, meshData: PathfinderMeshData) {
        for (const bufferName of Object.keys(BUFFER_TYPES) as Array<keyof PathfinderMeshBuffers>) {
            const bufferType = gl[BUFFER_TYPES[bufferName]];
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
            this[bufferName] = buffer;
        }
    }

    readonly bQuads: WebGLBuffer;
    readonly bVertexPositions: WebGLBuffer;
    readonly bVertexPathIDs: WebGLBuffer;
    readonly bVertexLoopBlinnData: WebGLBuffer;
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
        this.fpsLabel = unwrapNull(document.getElementById('pf-fps-label'));

        const canvas = document.getElementById('pf-canvas') as HTMLCanvasElement;
        const shaderLoader = new PathfinderShaderLoader;
        shaderLoader.load();
        this.view = Promise.all([shaderLoader.common, shaderLoader.shaders]).then(allShaders => {
            return new PathfinderView(this, canvas, allShaders[0], allShaders[1]);
        });

        this.loadFontButton = document.getElementById('pf-load-font-button') as HTMLInputElement;
        this.loadFontButton.addEventListener('change', () => this.loadFont(), false);

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as HTMLSelectElement;
        this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);
        this.updateAALevel();
    }

    loadFont() {
        const file = expectNotNull(this.loadFontButton.files, "No file selected!")[0];
        const reader = new FileReader;
        reader.addEventListener('loadend', () => {
            this.fontData = reader.result;
            this.fontLoaded();
        }, false);
        reader.readAsArrayBuffer(file);
    }

    updateAALevel() {
        const selectedOption = this.aaLevelSelect.selectedOptions[0];
        const aaType = unwrapUndef(selectedOption.dataset.pfType) as
            keyof AntialiasingStrategyTable;
        const aaLevel = parseInt(unwrapUndef(selectedOption.dataset.pfLevel));
        this.view.then(view => view.setAntialiasingOptions(aaType, aaLevel));
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
        }).then(response => response.text()).then(encodedMeshes => {
            this.meshes = new PathfinderMeshData(encodedMeshes);
            this.meshesReceived();
        });
    }

    meshesReceived() {
        this.view.then(view => {
            view.uploadPathData(TEXT.length);
            view.attachMeshes(this.meshes);
        })
    }

    updateTiming(newTime: number) {
        this.fpsLabel.innerHTML = `${newTime} ms`;
    }

    view: Promise<PathfinderView>;
    loadFontButton: HTMLInputElement;
    aaLevelSelect: HTMLSelectElement;
    fpsLabel: HTMLElement;
    fontData: ArrayBuffer;
    font: any;
    meshes: PathfinderMeshData;
}

class PathfinderShaderLoader {
    load() {
        this.common = window.fetch(COMMON_SHADER_URL).then(response => response.text());

        const shaderKeys = Object.keys(SHADER_URLS) as Array<keyof ShaderMap<string>>;
        let promises = [];
        for (const shaderKey of shaderKeys) {
            promises.push(Promise.all([
                window.fetch(SHADER_URLS[shaderKey].vertex).then(response => response.text()),
                window.fetch(SHADER_URLS[shaderKey].fragment).then(response => response.text()),
            ]).then(results => { return { vertex: results[0], fragment: results[1] } }));
        }

        this.shaders = Promise.all(promises).then(promises => {
            let shaderMap: Partial<ShaderMap<ShaderProgramSource>> = {};
            for (let keyIndex = 0; keyIndex < shaderKeys.length; keyIndex++)
                shaderMap[shaderKeys[keyIndex]] = promises[keyIndex];
            return shaderMap as ShaderMap<ShaderProgramSource>;
        });
    }

    common: Promise<string>;
    shaders: Promise<ShaderMap<ShaderProgramSource>>;
}

class PathfinderView {
    constructor(appController: AppController,
                canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        this.appController = appController;

        this.transform = glmatrix.mat4.create();

        this.canvas = canvas;
        this.canvas.addEventListener('wheel', event => this.onWheel(event), false);

        this.initContext();

        this.antialiasingStrategy = new NoAAStrategy(0);

        const shaderSource = this.compileShaders(commonShaderSource, shaderSources);
        this.shaderPrograms = this.linkShaders(shaderSource);

        window.addEventListener('resize', () => this.resizeToFit(), false);
        this.resizeToFit();
    }

    setAntialiasingOptions(aaType: keyof AntialiasingStrategyTable, aaLevel: number) {
        this.antialiasingStrategy = new (ANTIALIASING_STRATEGIES[aaType])(aaLevel);

        let canvas = this.canvas;
        this.antialiasingStrategy.init(this, { width: canvas.width, height: canvas.height });
        if (this.meshData != null)
            this.antialiasingStrategy.attachMeshes(this);

        this.setDirty();
    }

    initContext() {
        // Initialize the OpenGL context.
        this.gl = expectNotNull(this.canvas.getContext('webgl', { antialias: false, depth: true }),
                                "Failed to initialize WebGL! Check that your browser supports it.");
        this.drawBuffersExt = this.gl.getExtension('WEBGL_draw_buffers');
        this.colorBufferHalfFloatExt = this.gl.getExtension('EXT_color_buffer_half_float');
        this.instancedArraysExt = this.gl.getExtension('ANGLE_instanced_arrays');
        this.textureHalfFloatExt = this.gl.getExtension('OES_texture_half_float');
        this.timerQueryExt = this.gl.getExtension('EXT_disjoint_timer_query');
        this.vertexArrayObjectExt = this.gl.getExtension('OES_vertex_array_object');
        this.gl.getExtension('EXT_frag_depth');
        this.gl.getExtension('OES_element_index_uint');
        this.gl.getExtension('OES_texture_float');
        this.gl.getExtension('WEBGL_depth_texture');

        // Set up our timer query for profiling.
        this.timerQuery = this.timerQueryExt.createQueryEXT();

        // Upload quad buffers.
        this.quadPositionsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadPositionsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_POSITIONS, this.gl.STATIC_DRAW);
        this.quadTexCoordsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadTexCoordsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_TEX_COORDS, this.gl.STATIC_DRAW);
        this.quadElementsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.quadElementsBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, QUAD_ELEMENTS, this.gl.STATIC_DRAW);
    }

    compileShaders(commonSource: string, shaderSources: ShaderMap<ShaderProgramSource>):
                   ShaderMap<UnlinkedShaderProgram> {
        let shaders: Partial<ShaderMap<Partial<UnlinkedShaderProgram>>> = {};
        const shaderKeys = Object.keys(SHADER_URLS) as Array<keyof ShaderMap<string>>;

        for (const shaderKey of shaderKeys) {
            for (const typeName of ['vertex', 'fragment'] as Array<ShaderTypeName>) {
                const type = {
                    vertex: this.gl.VERTEX_SHADER,
                    fragment: this.gl.FRAGMENT_SHADER,
                }[typeName];

                const source = shaderSources[shaderKey][typeName];
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
            }
        }

        return shaders as ShaderMap<UnlinkedShaderProgram>;
    }

    linkShaders(shaders: ShaderMap<UnlinkedShaderProgram>): ShaderMap<PathfinderShaderProgram> {
        let shaderProgramMap: Partial<ShaderMap<PathfinderShaderProgram>> = {};
        for (const shaderName of Object.keys(shaders) as Array<keyof ShaderMap<string>>) {
            shaderProgramMap[shaderName] = new PathfinderShaderProgram(this.gl,
                                                                       shaderName,
                                                                       shaders[shaderName]);
        }
        return shaderProgramMap as ShaderMap<PathfinderShaderProgram>;
    }

    uploadPathData(pathCount: number) {
        const pathColors = new Uint8Array(4 * pathCount);
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[pathIndex * 4 + channel] = 0x00; // RGB
            pathColors[pathIndex * 4 + 3] = 0xff;           // alpha
        }

        this.pathColorsBufferTexture = new PathfinderBufferTexture(this.gl,
                                                                   pathColors,
                                                                   'uPathColors');
    }

    attachMeshes(meshes: PathfinderMeshData) {
        this.meshData = meshes;
        this.meshes = new PathfinderMeshBuffers(this.gl, meshes);
        this.antialiasingStrategy.attachMeshes(this);
        this.setDirty();
    }

    setDirty() {
        if (this.dirty)
            return;
        this.dirty = true;
        window.requestAnimationFrame(() => this.redraw());
    }

    // FIXME(pcwalton): This logic is all wrong.
    onWheel(event: WheelEvent) {
        if (event.ctrlKey) {
            // Zoom event: see https://developer.mozilla.org/en-US/docs/Web/Events/wheel
            const scaleFactor = 1.0 - event.deltaY * window.devicePixelRatio * SCALE_FACTOR;
            const scaleFactors = new Float32Array([scaleFactor, scaleFactor, 1.0]);
            glmatrix.mat4.scale(this.transform, this.transform, scaleFactors);
        } else {
            const delta = new Float32Array([
                -event.deltaX * window.devicePixelRatio,
                event.deltaY * window.devicePixelRatio,
                0.0
            ]);
            glmatrix.mat4.translate(this.transform, this.transform, delta);
        }

        this.setDirty();
    }

    resizeToFit() {
        const width = window.innerWidth;
        const height = window.scrollY + window.innerHeight -
            this.canvas.getBoundingClientRect().top;
        const devicePixelRatio = window.devicePixelRatio;

        const framebufferSize = {
            width: width * devicePixelRatio,
            height: height * devicePixelRatio,
        };

        this.canvas.style.width = width + 'px';
        this.canvas.style.height = height + 'px';
        this.canvas.width = framebufferSize.width;
        this.canvas.height = framebufferSize.height;

        this.antialiasingStrategy.init(this, framebufferSize);

        this.setDirty();
    }

    redraw() {
        if (this.meshes == null) {
            this.dirty = false;
            return;
        }

        // Start timing.
        if (this.timerQueryPollInterval == null)
            this.timerQueryExt.beginQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT, this.timerQuery);

        // Prepare for direct rendering.
        this.antialiasingStrategy.prepare(this);

        // Perform direct rendering (Loop-Blinn).
        this.renderDirect();

        // Antialias.
        this.antialiasingStrategy.resolve(this);

        // Finish timing and update the profile.
        this.updateTiming();

        // Clear dirty bit and finish.
        this.dirty = false;
    }

    updateTiming() {
        if (this.timerQueryPollInterval != null)
            return;

        this.timerQueryExt.endQueryEXT(this.timerQueryExt.TIME_ELAPSED_EXT);

        this.timerQueryPollInterval = window.setInterval(() => {
            if (this.timerQueryExt.getQueryObjectEXT(this.timerQuery,
                                                     this.timerQueryExt
                                                         .QUERY_RESULT_AVAILABLE_EXT) == 0) {
                return;
            }

            const elapsedTime =
                this.timerQueryExt.getQueryObjectEXT(this.timerQuery,
                                                     this.timerQueryExt.QUERY_RESULT_EXT);
            this.appController.updateTiming(elapsedTime / 1000000.0);

            window.clearInterval(this.timerQueryPollInterval!);
            this.timerQueryPollInterval = null;
        }, TIME_INTERVAL_DELAY);
    }

    setTransformUniform(uniforms: UniformMap) {
        this.gl.uniformMatrix4fv(uniforms.uTransform, false, this.transform);
    }

    setFramebufferSizeUniform(uniforms: UniformMap) {
        const currentViewport = this.gl.getParameter(this.gl.VIEWPORT);
        this.gl.uniform2i(uniforms.uFramebufferSize, currentViewport[2], currentViewport[3]);
    }

    renderDirect() {
        // Set up implicit cover state.
        this.gl.depthFunc(this.gl.GREATER);
        this.gl.depthMask(true);
        this.gl.enable(this.gl.DEPTH_TEST);
        this.gl.disable(this.gl.BLEND);

        // Set up the implicit cover interior VAO.
        const directInteriorProgram = this.shaderPrograms.directInterior;
        this.gl.useProgram(directInteriorProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
        this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPosition,
                                    2,
                                    this.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPathIDs);
        this.gl.vertexAttribPointer(directInteriorProgram.attributes.aPathID,
                                    1,
                                    this.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
        this.gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathID);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverInteriorIndices);

        // Draw direct interior parts.
        this.setTransformUniform(directInteriorProgram.uniforms);
        this.setFramebufferSizeUniform(directInteriorProgram.uniforms);
        this.pathColorsBufferTexture.bind(this.gl, directInteriorProgram.uniforms, 0);
        let indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                    this.gl.BUFFER_SIZE) / UINT32_SIZE;
        this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);

        // Set up direct curve state.
        this.gl.depthMask(false);
        this.gl.enable(this.gl.BLEND);
        this.gl.blendEquation(this.gl.FUNC_ADD);
        this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);

        // Set up the direct curve VAO.
        const directCurveProgram = this.shaderPrograms.directCurve;
        this.gl.useProgram(directCurveProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPositions);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aPosition,
                                    2,
                                    this.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexPathIDs);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aPathID,
                                    1,
                                    this.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.meshes.bVertexLoopBlinnData);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aTexCoord,
                                    2,
                                    this.gl.UNSIGNED_BYTE,
                                    false,
                                    B_LOOP_BLINN_DATA_SIZE,
                                    B_LOOP_BLINN_DATA_TEX_COORD_OFFSET);
        this.gl.vertexAttribPointer(directCurveProgram.attributes.aSign,
                                    1,
                                    this.gl.BYTE,
                                    false,
                                    B_LOOP_BLINN_DATA_SIZE,
                                    B_LOOP_BLINN_DATA_SIGN_OFFSET);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aTexCoord);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aPathID);
        this.gl.enableVertexAttribArray(directCurveProgram.attributes.aSign);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.meshes.coverCurveIndices);

        // Draw direct curve parts.
        this.setTransformUniform(directCurveProgram.uniforms);
        this.setFramebufferSizeUniform(directCurveProgram.uniforms);
        this.pathColorsBufferTexture.bind(this.gl, directCurveProgram.uniforms, 0);
        indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                this.gl.BUFFER_SIZE) / UINT32_SIZE;
        this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);
    }

    canvas: HTMLCanvasElement;

    gl: WebGLRenderingContext;

    colorBufferHalfFloatExt: any;
    drawBuffersExt: any;
    instancedArraysExt: any;
    textureHalfFloatExt: any;
    timerQueryExt: any;
    vertexArrayObjectExt: any;

    antialiasingStrategy: AntialiasingStrategy;

    shaderPrograms: ShaderMap<PathfinderShaderProgram>;

    meshes: PathfinderMeshBuffers;
    meshData: PathfinderMeshData;

    timerQuery: WebGLQuery;
    timerQueryPollInterval: number | null;

    pathColorsBufferTexture: PathfinderBufferTexture;

    quadPositionsBuffer: WebGLBuffer;
    quadTexCoordsBuffer: WebGLBuffer;
    quadElementsBuffer: WebGLBuffer;

    transform: Matrix4D;

    appController: AppController;

    dirty: boolean;
}

class PathfinderShaderProgram {
    constructor(gl: WebGLRenderingContext,
                programName: string,
                unlinkedShaderProgram: UnlinkedShaderProgram) {
        this.program = expectNotNull(gl.createProgram(), "Failed to create shader program!");
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
            const uniformName = unwrapNull(gl.getActiveUniform(this.program, uniformIndex)).name;
            uniforms[uniformName] = expectNotNull(gl.getUniformLocation(this.program, uniformName),
                                                  `Didn't find uniform "${uniformName}"!`);
        }
        for (let attributeIndex = 0; attributeIndex < attributeCount; attributeIndex++) {
            const attributeName = unwrapNull(gl.getActiveAttrib(this.program, attributeIndex)).name;
            attributes[attributeName] = attributeIndex;
        }

        this.uniforms = uniforms;
        this.attributes = attributes;
    }

    readonly uniforms: UniformMap;
    readonly attributes: AttributeMap;
    readonly program: WebGLProgram;
}

class PathfinderBufferTexture {
    constructor(gl: WebGLRenderingContext,
                data: Float32Array | Uint8Array,
                uniformName: string) {
        const pixelCount = Math.ceil(data.length / 4);
        const width = Math.ceil(Math.sqrt(pixelCount));
        const height = Math.ceil(pixelCount / width);
        this.size = { width: width, height: height };

        this.uniformName = uniformName;

        // Pad out with zeroes as necessary.
        //
        // FIXME(pcwalton): Do this earlier to save a copy here.
        const elementCount = width * height * 4;
        if (data.length != elementCount) {
            const newData = new Float32Array(elementCount);
            newData.set(data);
            data = newData;
        }

        this.texture = expectNotNull(gl.createTexture(), "Failed to create texture!");
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        const glType = data instanceof Float32Array ? gl.FLOAT : gl.UNSIGNED_BYTE;
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, glType, data);

        setTextureParameters(gl, gl.NEAREST);
    }

    bind(gl: WebGLRenderingContext, uniforms: UniformMap, textureUnit: number) {
        gl.activeTexture(gl.TEXTURE0 + textureUnit);
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        gl.uniform2i(uniforms[`${this.uniformName}Dimensions`], this.size.width, this.size.height);
        gl.uniform1i(uniforms[this.uniformName], textureUnit);
    }

    readonly texture: WebGLTexture;
    readonly size: Size2D;
    readonly uniformName: string;
}

class NoAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.framebufferSize = { width: 0, height: 0 };
    }

    init(view: PathfinderView, framebufferSize: Size2D) {
        this.framebufferSize = framebufferSize;
    }

    attachMeshes(view: PathfinderView) {}

    prepare(view: PathfinderView) {
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);

        // Clear.
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    resolve(view: PathfinderView) {}

    framebufferSize: Size2D;
}

class SSAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.level = level;
        this.canvasFramebufferSize = { width: 0, height: 0 };
        this.supersampledFramebufferSize = { width: 0, height: 0 };
    }

    init(view: PathfinderView, framebufferSize: Size2D) {
        this.canvasFramebufferSize = framebufferSize;
        this.supersampledFramebufferSize = {
            width: framebufferSize.width * 2,
            height: framebufferSize.height * (this.level == 2 ? 1 : 2),
        };

        this.supersampledColorTexture = unwrapNull(view.gl.createTexture());
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.supersampledColorTexture);
        view.gl.texImage2D(view.gl.TEXTURE_2D,
                           0,
                           view.gl.RGBA,
                           this.supersampledFramebufferSize.width,
                           this.supersampledFramebufferSize.height,
                           0,
                           view.gl.RGBA,
                           view.gl.UNSIGNED_BYTE,
                           null);
        setTextureParameters(view.gl, view.gl.LINEAR);

        this.supersampledDepthTexture =
            createFramebufferDepthTexture(view.gl, this.supersampledFramebufferSize);

        this.supersampledFramebuffer = createFramebuffer(view.gl,
                                                         view.drawBuffersExt,
                                                         [this.supersampledColorTexture],
                                                         this.supersampledDepthTexture);

        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
    }

    attachMeshes(view: PathfinderView) {}

    prepare(view: PathfinderView) {
        const size = this.supersampledFramebufferSize;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.supersampledFramebuffer);
        view.gl.viewport(0, 0, size.width, size.height);

        // Clear.
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    resolve(view: PathfinderView) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
        view.gl.viewport(0, 0, view.canvas.width, view.canvas.height);
        view.gl.disable(view.gl.DEPTH_TEST);

        // Set up the blit program VAO.
        const blitProgram = view.shaderPrograms.blit;
        view.gl.useProgram(blitProgram.program);
        initQuadVAO(view, blitProgram.attributes);

        // Resolve framebuffer.
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.supersampledColorTexture);
        view.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
    }

    level: number;
    canvasFramebufferSize: Readonly<Size2D>;
    supersampledFramebufferSize: Readonly<Size2D>;
    supersampledColorTexture: WebGLTexture;
    supersampledDepthTexture: WebGLTexture;
    supersampledFramebuffer: WebGLFramebuffer;
}

class ECAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.framebufferSize = { width: 0, height: 0 };
    }

    init(view: PathfinderView, framebufferSize: Size2D) {
        this.framebufferSize = framebufferSize;

        this.initDirectFramebuffer(view);
        this.initEdgeDetectFramebuffer(view);
        this.initAAAlphaFramebuffer(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);

        this.createEdgeDetectVAO(view);
    }

    attachMeshes(view: PathfinderView) {
        const bVertexPositions = new Float32Array(view.meshData.bVertexPositions);
        const bVertexPathIDs = new Uint8Array(view.meshData.bVertexPathIDs);
        this.bVertexPositionBufferTexture = new PathfinderBufferTexture(view.gl,
                                                                        bVertexPositions,
                                                                        'uBVertexPosition');
        this.bVertexPathIDBufferTexture = new PathfinderBufferTexture(view.gl,
                                                                      bVertexPathIDs,
                                                                      'uBVertexPathID');

        this.createEdgeDetectVAO(view);
        this.createCoverVAO(view);
        this.createLineVAOs(view);
        this.createCurveVAOs(view);
        this.createResolveVAO(view);
    }

    initDirectFramebuffer(view: PathfinderView) {
        this.directColorTexture = createFramebufferColorTexture(view.gl, this.framebufferSize);
        this.directPathIDTexture = createFramebufferColorTexture(view.gl, this.framebufferSize);
        this.directDepthTexture = createFramebufferDepthTexture(view.gl, this.framebufferSize);
        this.directFramebuffer =
            createFramebuffer(view.gl,
                              view.drawBuffersExt,
                              [this.directColorTexture, this.directPathIDTexture],
                              this.directDepthTexture);
    }

    initEdgeDetectFramebuffer(view: PathfinderView) {
        this.bgColorTexture = createFramebufferColorTexture(view.gl, this.framebufferSize);
        this.fgColorTexture = createFramebufferColorTexture(view.gl, this.framebufferSize);
        this.aaDepthTexture = createFramebufferDepthTexture(view.gl, this.framebufferSize);
        this.edgeDetectFramebuffer = createFramebuffer(view.gl,
                                                       view.drawBuffersExt,
                                                       [this.bgColorTexture, this.fgColorTexture],
                                                       this.aaDepthTexture);
    }

    initAAAlphaFramebuffer(view: PathfinderView) {
        this.aaAlphaTexture = unwrapNull(view.gl.createTexture());
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.aaAlphaTexture);
        view.gl.texImage2D(view.gl.TEXTURE_2D,
                           0,
                           view.gl.RGB,
                           this.framebufferSize.width,
                           this.framebufferSize.height,
                           0,
                           view.gl.RGB,
                           view.textureHalfFloatExt.HALF_FLOAT_OES,
                           null);
        setTextureParameters(view.gl, view.gl.NEAREST);

        this.aaFramebuffer = createFramebuffer(view.gl,
                                               view.drawBuffersExt,
                                               [this.aaAlphaTexture],
                                               this.aaDepthTexture);
    }

    createEdgeDetectVAO(view: PathfinderView) {
        this.edgeDetectVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);

        const edgeDetectProgram = view.shaderPrograms.ecaaEdgeDetect;
        view.gl.useProgram(edgeDetectProgram.program);
        initQuadVAO(view, edgeDetectProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    createCoverVAO(view: PathfinderView) {
        this.coverVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const coverProgram = view.shaderPrograms.ecaaCover;
        const attributes = coverProgram.attributes;
        view.gl.useProgram(coverProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.meshes.bQuads);
        view.gl.vertexAttribPointer(attributes.aUpperPointIndices,
                                    4,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    B_QUAD_SIZE,
                                    B_QUAD_UPPER_INDICES_OFFSET);
        view.gl.vertexAttribPointer(attributes.aLowerPointIndices,
                                    4,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    B_QUAD_SIZE,
                                    B_QUAD_LOWER_INDICES_OFFSET);
        view.gl.enableVertexAttribArray(attributes.aQuadPosition);
        view.gl.enableVertexAttribArray(attributes.aUpperPointIndices);
        view.gl.enableVertexAttribArray(attributes.aLowerPointIndices);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aUpperPointIndices, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLowerPointIndices, 1);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    createLineVAOs(view: PathfinderView) {
        const lineProgram = view.shaderPrograms.ecaaLine;
        const attributes = lineProgram.attributes;

        const vaos: Partial<UpperAndLower<WebGLVertexArrayObject>> = {};
        for (const direction of ['upper', 'lower'] as Array<'upper' | 'lower'>) {
            vaos[direction] = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vaos[direction]);

            const lineIndexBuffer = {
                upper: view.meshes.edgeUpperLineIndices,
                lower: view.meshes.edgeLowerLineIndices,
            }[direction];

            view.gl.useProgram(lineProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, lineIndexBuffer);
            view.gl.vertexAttribPointer(attributes.aLineIndices,
                                        4,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);
            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aLineIndices);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLineIndices, 1);
            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAOs = vaos as UpperAndLower<WebGLVertexArrayObject>;
    }

    createCurveVAOs(view: PathfinderView) {
        const curveProgram = view.shaderPrograms.ecaaCurve;
        const attributes = curveProgram.attributes;

        const vaos: Partial<UpperAndLower<WebGLVertexArrayObject>> = {};
        for (const direction of ['upper', 'lower'] as Array<'upper' | 'lower'>) {
            vaos[direction] = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vaos[direction]);

            const curveIndexBuffer = {
                upper: view.meshes.edgeUpperCurveIndices,
                lower: view.meshes.edgeLowerCurveIndices,
            }[direction];

            view.gl.useProgram(curveProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curveIndexBuffer);
            view.gl.vertexAttribPointer(attributes.aCurveEndpointIndices,
                                        4,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        UINT32_SIZE * 4,
                                        0);
            view.gl.vertexAttribPointer(attributes.aCurveControlPointIndex,
                                        2,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        UINT32_SIZE * 4,
                                        UINT32_SIZE * 2);
            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aCurveEndpointIndices);
            view.gl.enableVertexAttribArray(attributes.aCurveControlPointIndex);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCurveEndpointIndices, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCurveControlPointIndex, 1);
            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAOs = vaos as UpperAndLower<WebGLVertexArrayObject>;
    }

    createResolveVAO(view: PathfinderView) {
        this.resolveVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = view.shaderPrograms.ecaaResolve;
        view.gl.useProgram(resolveProgram.program);
        initQuadVAO(view, resolveProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    prepare(view: PathfinderView) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.directFramebuffer);
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);

        // Clear out the color and depth textures.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.NONE,
        ]);
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);

        // Clear out the path ID texture.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.NONE,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);

        // Render to both textures.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
    }

    resolve(view: PathfinderView) {
        // Detect edges.
        this.detectEdges(view);

        // Conservatively cover.
        //if (view.timerQueryPollInterval == null)
        //    view.timerQueryExt.beginQueryEXT(view.timerQueryExt.TIME_ELAPSED_EXT, view.timerQuery);
        this.cover(view);

        // Antialias.
        this.antialiasLines(view);
        this.antialiasCurves(view);

        //if (view.timerQueryPollInterval == null)
        //    view.timerQueryExt.endQueryEXT(view.timerQueryExt.TIME_ELAPSED_EXT);

        // Resolve the antialiasing.
        this.resolveAA(view);
    }

    detectEdges(view: PathfinderView) {
        // Set state for edge detection.
        const edgeDetectProgram = view.shaderPrograms.ecaaEdgeDetect;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.edgeDetectFramebuffer);
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);

        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);

        view.gl.depthMask(true);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
        view.gl.disable(view.gl.BLEND);

        view.gl.clearDepth(0.0);
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);

        // Perform edge detection.
        view.gl.useProgram(edgeDetectProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);
        view.gl.uniform2i(edgeDetectProgram.uniforms.uFramebufferSize,
                          this.framebufferSize.width,
                          this.framebufferSize.height);
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.directColorTexture);
        view.gl.uniform1i(edgeDetectProgram.uniforms.uColor, 0);
        view.gl.activeTexture(view.gl.TEXTURE1);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.directPathIDTexture);
        view.gl.uniform1i(edgeDetectProgram.uniforms.uPathID, 1);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    cover(view: PathfinderView) {
        // Set state for conservative coverage.
        const coverProgram = view.shaderPrograms.ecaaCover;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.aaFramebuffer);
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);

        view.gl.depthMask(false);
        //view.gl.depthFunc(view.gl.EQUAL);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
        view.gl.blendEquation(view.gl.FUNC_ADD);
        view.gl.blendFunc(view.gl.ONE, view.gl.ONE);
        view.gl.enable(view.gl.BLEND);

        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);

        // Conservatively cover.
        view.gl.useProgram(coverProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        const uniforms = coverProgram.uniforms;
        view.setTransformUniform(uniforms);
        view.setFramebufferSizeUniform(uniforms);
        this.bVertexPositionBufferTexture.bind(view.gl, uniforms, 0);
        this.bVertexPathIDBufferTexture.bind(view.gl, uniforms, 1);
        view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                           6,
                                                           view.gl.UNSIGNED_BYTE,
                                                           0,
                                                           view.meshData.bQuadCount);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    setAAState(view: PathfinderView) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.aaFramebuffer);
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);

        view.gl.depthMask(false);
        //view.gl.depthFunc(view.gl.EQUAL);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
        view.gl.blendEquation(view.gl.FUNC_REVERSE_SUBTRACT);
        view.gl.blendFunc(view.gl.ONE, view.gl.ONE);
        view.gl.enable(view.gl.BLEND);
    }

    setAAUniforms(view: PathfinderView, uniforms: UniformMap) {
        view.setTransformUniform(uniforms);
        view.setFramebufferSizeUniform(uniforms);
        this.bVertexPositionBufferTexture.bind(view.gl, uniforms, 0);
        this.bVertexPathIDBufferTexture.bind(view.gl, uniforms, 1);
    }

    antialiasLines(view: PathfinderView) {
        this.setAAState(view);

        const lineProgram = view.shaderPrograms.ecaaLine;
        view.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of ['upper', 'lower'] as Array<keyof UpperAndLower<void>>) {
            view.vertexArrayObjectExt.bindVertexArrayOES(this.lineVAOs[direction]);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);
            const count = {
                upper: view.meshData.edgeUpperLineIndexCount,
                lower: view.meshData.edgeLowerLineIndexCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    antialiasCurves(view: PathfinderView) {
        this.setAAState(view);

        const curveProgram = view.shaderPrograms.ecaaCurve;
        view.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of ['upper', 'lower'] as Array<keyof UpperAndLower<void>>) {
            view.vertexArrayObjectExt.bindVertexArrayOES(this.curveVAOs[direction]);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);
            const count = {
                upper: view.meshData.edgeUpperCurveIndexCount,
                lower: view.meshData.edgeLowerCurveIndexCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    resolveAA(view: PathfinderView) {
        // Set state for ECAA resolve.
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);
        view.gl.disable(view.gl.DEPTH_TEST);
        view.gl.disable(view.gl.BLEND);
        view.drawBuffersExt.drawBuffersWEBGL([view.gl.BACK]);

        // Resolve.
        const resolveProgram = view.shaderPrograms.ecaaResolve;
        view.gl.useProgram(resolveProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);
        view.setFramebufferSizeUniform(resolveProgram.uniforms);
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.bgColorTexture);
        view.gl.uniform1i(resolveProgram.uniforms.uBGColor, 0);
        view.gl.activeTexture(view.gl.TEXTURE1);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.fgColorTexture);
        view.gl.uniform1i(resolveProgram.uniforms.uFGColor, 1);
        view.gl.activeTexture(view.gl.TEXTURE2);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.aaAlphaTexture);
        view.gl.uniform1i(resolveProgram.uniforms.uAAAlpha, 2);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    bVertexPositionBufferTexture: PathfinderBufferTexture;
    bVertexPathIDBufferTexture: PathfinderBufferTexture;
    directColorTexture: WebGLTexture;
    directPathIDTexture: WebGLTexture;
    directDepthTexture: WebGLTexture;
    directFramebuffer: WebGLFramebuffer;
    bgColorTexture: WebGLTexture;
    fgColorTexture: WebGLTexture;
    aaDepthTexture: WebGLTexture;
    aaAlphaTexture: WebGLTexture;
    edgeDetectFramebuffer: WebGLFramebuffer;
    aaFramebuffer: WebGLFramebuffer;
    edgeDetectVAO: WebGLVertexArrayObject;
    coverVAO: WebGLVertexArrayObject;
    lineVAOs: UpperAndLower<WebGLVertexArrayObject>;
    curveVAOs: UpperAndLower<WebGLVertexArrayObject>;
    resolveVAO: WebGLVertexArrayObject;
    framebufferSize: Size2D;
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    ecaa: typeof ECAAStrategy;
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    ecaa: ECAAStrategy,
};

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
