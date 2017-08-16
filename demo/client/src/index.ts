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
};

interface UnlinkedShaderProgram {
    vertex: WebGLShader;
    fragment: WebGLShader;
}

type Matrix4D = number[];

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
}

interface UniformMap {
    [uniformName: string]: WebGLUniformLocation;
}

interface AttributeMap {
    [attributeName: string]: number;
}

interface AntialiasingStrategy {
    // Prepares any OpenGL data. This is only called on startup and canvas resize.
    init(gl: WebGLRenderingContext, framebufferSize: Size2D): void;

    // Called before direct rendering.
    //
    // Typically, this redirects direct rendering to a framebuffer of some sort.
    prepare(view: PathfinderView): void;

    // Called after direct rendering.
    //
    // This usually performs the actual antialiasing and blits to the real framebuffer.
    resolve(view: PathfinderView, shaders: ShaderMap<PathfinderShaderProgram>): void;

    // Returns the size of the framebuffer for direct rendering.
    //
    // For supersampling-based techniques, this may be larger than the actual framebuffer.
    getFramebufferSize(): Size2D;
}

type ShaderType = number;

type ShaderTypeName = 'vertex' | 'fragment';

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

function setTextureParameters(gl: WebGLRenderingContext, filter: number) {
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
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
            const buffer = expectNotNull(gl.createBuffer(), "Failed to create buffer!");
            gl.bindBuffer(bufferType, buffer);
            gl.bufferData(bufferType, meshData[bufferName], gl.STATIC_DRAW);
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
        this.view.setAntialiasingOptions(aaType, aaLevel);
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
        this.view.uploadPathData(TEXT.length);
        this.view.attachMeshes(this.meshes);
    }

    view: PathfinderView;
    loadFontButton: HTMLInputElement;
    aaLevelSelect: HTMLSelectElement;
    fontData: ArrayBuffer;
    font: any;
    meshes: PathfinderMeshData;
}

class PathfinderView {
    constructor(canvas: HTMLCanvasElement) {
        this.canvas = canvas;

        this.initContext();

        this.antialiasingStrategy = new NoAAStrategy(0);

        this.shaderProgramsPromise = this.loadShaders().then(shaders => this.linkShaders(shaders));

        window.addEventListener('resize', () => this.resizeToFit(), false);
        this.resizeToFit();
    }

    setAntialiasingOptions(aaType: keyof AntialiasingStrategyTable, aaLevel: number) {
        this.antialiasingStrategy = new (ANTIALIASING_STRATEGIES[aaType])(aaLevel);

        let canvas = this.canvas;
        this.antialiasingStrategy.init(this.gl, { width: canvas.width, height: canvas.height });

        this.setDirty();
    }

    initContext() {
        // Initialize the OpenGL context.
        this.gl = expectNotNull(this.canvas.getContext('webgl', { antialias: false, depth: true }),
                                "Failed to initialize WebGL! Check that your browser supports it.");
        this.gl.getExtension('EXT_frag_depth');
        this.gl.getExtension('OES_element_index_uint');
        this.gl.getExtension('WEBGL_depth_texture');
        this.gl.getExtension('WEBGL_draw_buffers');

        // Upload quad buffers.
        this.quadPositionsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadPositionsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_POSITIONS, this.gl.STATIC_DRAW);
        this.quadTexCoordsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.quadTexCoordsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, QUAD_TEX_COORDS, this.gl.STATIC_DRAW);
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

    uploadPathData(pathCount: number) {
        const pathColors = new Uint8Array(4 * pathCount);
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[pathIndex * 4 + channel] = 0x00; // RGB
            pathColors[pathIndex * 4 + 3] = 0xff;           // alpha
        }

        this.pathColorsBufferTexture = new PathfinderBufferTexture(this.gl, pathColors);
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

        const framebufferSize = {
            width: width * devicePixelRatio,
            height: height * devicePixelRatio,
        };

        this.canvas.style.width = width + 'px';
        this.canvas.style.height = height + 'px';
        this.canvas.width = framebufferSize.width;
        this.canvas.height = framebufferSize.height;

        this.antialiasingStrategy.init(this.gl, framebufferSize);

        this.setDirty();
    }

    redraw() {
        this.shaderProgramsPromise.then((shaderPrograms: ShaderMap<PathfinderShaderProgram>) => {
            if (this.meshes == null) {
                this.dirty = false;
                return;
            }

            // Prepare for direct rendering.
            this.antialiasingStrategy.prepare(this);

            // Clear.
            this.gl.clearColor(1.0, 1.0, 1.0, 1.0);
            this.gl.clearDepth(0.0);
            this.gl.depthMask(true);
            this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);

            // Perform direct rendering (Loop-Blinn).
            this.renderDirect(shaderPrograms);

            // Antialias.
            this.antialiasingStrategy.resolve(this, shaderPrograms);

            // Clear dirty bit and finish.
            this.dirty = false;
        });
    }

    renderDirect(shaderPrograms: ShaderMap<PathfinderShaderProgram>) {
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
        this.gl.activeTexture(this.gl.TEXTURE0);
        this.gl.bindTexture(this.gl.TEXTURE_2D, this.pathColorsBufferTexture.texture);
        this.gl.uniformMatrix4fv(directInteriorProgram.uniforms.uTransform, false, IDENTITY);
        this.gl.uniform2i(directInteriorProgram.uniforms.uFramebufferSize,
                          this.canvas.width,
                          this.canvas.height);
        this.gl.uniform2i(directInteriorProgram.uniforms.uPathColorsDimensions,
                          this.pathColorsBufferTexture.size.width,
                          this.pathColorsBufferTexture.size.height);
        this.gl.uniform1i(directInteriorProgram.uniforms.uPathColors, 0);
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
        this.gl.activeTexture(this.gl.TEXTURE0);
        this.gl.bindTexture(this.gl.TEXTURE_2D, this.pathColorsBufferTexture.texture);
        this.gl.uniformMatrix4fv(directCurveProgram.uniforms.uTransform, false, IDENTITY);
        this.gl.uniform2i(directCurveProgram.uniforms.uFramebufferSize,
                          this.canvas.width,
                          this.canvas.height);
        this.gl.uniform2i(directCurveProgram.uniforms.uPathColorsDimensions,
                          this.pathColorsBufferTexture.size.width,
                          this.pathColorsBufferTexture.size.height);
        this.gl.uniform1i(directCurveProgram.uniforms.uPathColors, 0);
        indexCount = this.gl.getBufferParameter(this.gl.ELEMENT_ARRAY_BUFFER,
                                                this.gl.BUFFER_SIZE) / UINT32_SIZE;
        this.gl.drawElements(this.gl.TRIANGLES, indexCount, this.gl.UNSIGNED_INT, 0);
    }

    canvas: HTMLCanvasElement;
    gl: WebGLRenderingContext;
    antialiasingStrategy: AntialiasingStrategy;
    shaderProgramsPromise: Promise<ShaderMap<PathfinderShaderProgram>>;
    meshes: PathfinderMeshBuffers;
    pathColorsBufferTexture: PathfinderBufferTexture;
    quadPositionsBuffer: WebGLBuffer;
    quadTexCoordsBuffer: WebGLBuffer;
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
    constructor(gl: WebGLRenderingContext, data: Uint8Array) {
        const pixelCount = Math.ceil(data.length / 4);
        const width = Math.ceil(Math.sqrt(pixelCount));
        const height = Math.ceil(pixelCount / width);
        this.size = { width: width, height: height };

        this.texture = expectNotNull(gl.createTexture(), "Failed to create texture!");
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
        setTextureParameters(gl, gl.NEAREST);
    }

    readonly texture: WebGLTexture;
    readonly size: Size2D;
}

class NoAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.framebufferSize = { width: 0, height: 0 };
    }

    init(gl: WebGLRenderingContext, framebufferSize: Size2D) {
        this.framebufferSize = framebufferSize;
    }

    prepare(view: PathfinderView) {
        view.gl.viewport(0, 0, this.framebufferSize.width, this.framebufferSize.height);
    }

    resolve(view: PathfinderView, shaders: ShaderMap<PathfinderShaderProgram>) {}

    getFramebufferSize() {
        return this.framebufferSize;
    }

    framebufferSize: Size2D;
}

class SSAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.level = level;
        this.canvasFramebufferSize = { width: 0, height: 0 };
        this.supersampledFramebufferSize = { width: 0, height: 0 };
    }

    init(gl: WebGLRenderingContext, framebufferSize: Size2D) {
        this.canvasFramebufferSize = framebufferSize;
        this.supersampledFramebufferSize = {
            width: framebufferSize.width * 2,
            height: framebufferSize.height * (this.level == 2 ? 1 : 2),
        };

        this.supersampledColorTexture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, this.supersampledColorTexture);
        gl.texImage2D(gl.TEXTURE_2D,
                      0,
                      gl.RGBA,
                      this.supersampledFramebufferSize.width,
                      this.supersampledFramebufferSize.height,
                      0,
                      gl.RGBA,
                      gl.UNSIGNED_BYTE,
                      null);
        setTextureParameters(gl, gl.LINEAR);

        this.supersampledDepthTexture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, this.supersampledDepthTexture);
        gl.texImage2D(gl.TEXTURE_2D,
                      0,
                      gl.DEPTH_COMPONENT,
                      this.supersampledFramebufferSize.width,
                      this.supersampledFramebufferSize.height,
                      0,
                      gl.DEPTH_COMPONENT,
                      gl.UNSIGNED_INT,
                      null);
        setTextureParameters(gl, gl.NEAREST);

        this.supersampledFramebuffer = unwrapNull(gl.createFramebuffer());
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.supersampledFramebuffer);
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                gl.COLOR_ATTACHMENT0,
                                gl.TEXTURE_2D,
                                this.supersampledColorTexture,
                                0);
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                gl.DEPTH_ATTACHMENT,
                                gl.TEXTURE_2D,
                                this.supersampledDepthTexture,
                                0);
        assert(gl.checkFramebufferStatus(gl.FRAMEBUFFER) == gl.FRAMEBUFFER_COMPLETE,
               "The SSAA framebuffer was incomplete!");

        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    }

    prepare(view: PathfinderView) {
        const size = this.supersampledFramebufferSize;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.supersampledFramebuffer);
        view.gl.viewport(0, 0, size.width, size.height);
    }

    resolve(view: PathfinderView, shaders: ShaderMap<PathfinderShaderProgram>) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
        view.gl.viewport(0, 0, view.canvas.width, view.canvas.height);
        view.gl.disable(view.gl.DEPTH_TEST);

        // Set up the blit program VAO.
        const blitProgram = shaders.blit;
        view.gl.useProgram(blitProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(blitProgram.attributes.aPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadTexCoordsBuffer);
        view.gl.vertexAttribPointer(blitProgram.attributes.aTexCoord,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        view.gl.enableVertexAttribArray(blitProgram.attributes.aPosition);
        view.gl.enableVertexAttribArray(blitProgram.attributes.aTexCoord);

        // Resolve framebuffer.
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.supersampledColorTexture);
        view.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        view.gl.drawArrays(view.gl.TRIANGLE_STRIP, 0, 4);
    }

    getFramebufferSize() {
        return this.supersampledFramebufferSize;
    }

    level: number;
    canvasFramebufferSize: Readonly<Size2D>;
    supersampledFramebufferSize: Readonly<Size2D>;
    supersampledColorTexture: WebGLTexture;
    supersampledDepthTexture: WebGLTexture;
    supersampledFramebuffer: WebGLFramebuffer;
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
};

function main() {
    const controller = new AppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
