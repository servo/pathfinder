// pathfinder/shaders/gles2/demo-3d-monument.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders the monument surface in the 3D demo.

precision mediump float;

uniform vec3 uLightPosition;
uniform vec3 uAmbientColor;
uniform vec3 uDiffuseColor;
uniform vec3 uSpecularColor;
uniform float uShininess;

uniform vec3 uNormal;

varying vec3 vPosition;

void main() {
    vec3 normal = normalize(uNormal);
    vec3 lightDirection = normalize(uLightPosition - vPosition);

    float lambertian = max(dot(lightDirection, normal), 0.0);
    float specular = 0.0;

    if (lambertian > 0.0) {
        vec3 viewDirection = normalize(-vPosition);
        vec3 halfDirection = normalize(lightDirection + viewDirection);
        float specularAngle = max(dot(halfDirection, normal), 0.0);
        specular = pow(specularAngle, uShininess);
    }

    vec3 color = uAmbientColor + lambertian * uDiffuseColor + specular * uSpecularColor;
    gl_FragColor = vec4(color, 1.0);
}
