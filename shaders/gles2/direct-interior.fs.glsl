// pathfinder/shaders/gles2/direct-interior.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

varying vec4 vColor;
varying vec2 vPathID;

void main() {
    gl_FragData[0] = vColor;
    gl_FragData[1] = vec4(vPathID, 1.0, 1.0);
}
