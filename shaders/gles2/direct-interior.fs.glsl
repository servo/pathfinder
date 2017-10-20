// pathfinder/shaders/gles2/direct-interior.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

varying vec4 vColor;
varying vec2 vPathID;

void main() {
    gl_FragData[0] = vColor;
    gl_FragData[1] = vec4(vPathID, 0.0, 1.0);
}
