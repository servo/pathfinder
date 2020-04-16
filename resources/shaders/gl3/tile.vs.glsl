#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;
precision highp sampler2D;

uniform mat4 uTransform;
uniform vec2 uTileSize;
uniform sampler2D uTextureMetadata;
uniform ivec2 uTextureMetadataSize;

in ivec2 aTileOffset;
in ivec2 aTileOrigin;
in uvec2 aMaskTexCoord0;
in uvec2 aMaskTexCoord1;
in ivec2 aMaskBackdrop;
in int aColor;

out vec3 vMaskTexCoord0;
out vec3 vMaskTexCoord1;
out vec2 vColorTexCoord0;
out vec4 vBaseColor;

void main(){
    vec2 tileOrigin = vec2(aTileOrigin), tileOffset = vec2(aTileOffset);
    vec2 position =(tileOrigin + tileOffset)* uTileSize;

    vec2 maskTexCoord0 =(vec2(aMaskTexCoord0)+ tileOffset)/ 256.0;
    vec2 maskTexCoord1 =(vec2(aMaskTexCoord1)+ tileOffset)/ 256.0;

    vec2 textureMetadataScale = vec2(1.0)/ vec2(uTextureMetadataSize);
    vec2 metadataEntryCoord = vec2(aColor % 128 * 4, aColor / 128);
    vec2 colorTexMatrix0Coord =(metadataEntryCoord + vec2(0.5, 0.5))* textureMetadataScale;
    vec2 colorTexOffsetsCoord =(metadataEntryCoord + vec2(1.5, 0.5))* textureMetadataScale;
    vec2 baseColorCoord =(metadataEntryCoord + vec2(2.5, 0.5))* textureMetadataScale;
    vec4 colorTexMatrix0 = texture(uTextureMetadata, colorTexMatrix0Coord);
    vec4 colorTexOffsets = texture(uTextureMetadata, colorTexOffsetsCoord);
    vec4 baseColor = texture(uTextureMetadata, baseColorCoord);

    vColorTexCoord0 = mat2(colorTexMatrix0)* position + colorTexOffsets . xy;
    vMaskTexCoord0 = vec3(maskTexCoord0, float(aMaskBackdrop . x));
    vMaskTexCoord1 = vec3(maskTexCoord1, float(aMaskBackdrop . y));
    vBaseColor = baseColor;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

