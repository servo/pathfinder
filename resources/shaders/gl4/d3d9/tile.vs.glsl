#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












#extension GL_GOOGLE_include_directive : enable

precision highp float;
















vec4 fetchUnscaled(sampler2D srcTexture, vec2 scale, vec2 originCoord, int entry){
    return texture(srcTexture,(originCoord + vec2(0.5)+ vec2(entry, 0))* scale);
}

void computeTileVaryings(vec2 position,
                         int colorEntry,
                         sampler2D textureMetadata,
                         ivec2 textureMetadataSize,
                         out vec2 outColorTexCoord0,
                         out vec4 outBaseColor,
                         out vec4 outFilterParams0,
                         out vec4 outFilterParams1,
                         out vec4 outFilterParams2,
                         out vec4 outFilterParams3,
                         out vec4 outFilterParams4,
                         out int outCtrl){
    vec2 metadataScale = vec2(1.0)/ vec2(textureMetadataSize);
    vec2 metadataEntryCoord = vec2(colorEntry % 128 * 10, colorEntry / 128);
    vec4 colorTexMatrix0 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 0);
    vec4 colorTexOffsets = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 1);
    vec4 baseColor = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 2);
    vec4 filterParams0 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 3);
    vec4 filterParams1 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 4);
    vec4 filterParams2 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 5);
    vec4 filterParams3 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 6);
    vec4 filterParams4 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 7);
    vec4 extra = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 8);
    outColorTexCoord0 = mat2(colorTexMatrix0)* position + colorTexOffsets . xy;
    outBaseColor = baseColor;
    outFilterParams0 = filterParams0;
    outFilterParams1 = filterParams1;
    outFilterParams2 = filterParams2;
    outFilterParams3 = filterParams3;
    outFilterParams4 = filterParams4;
    outCtrl = int(extra . x);
}


uniform mat4 uTransform;
uniform vec2 uTileSize;
uniform sampler2D uTextureMetadata;
uniform ivec2 uTextureMetadataSize;
uniform sampler2D uZBuffer;
uniform ivec2 uZBufferSize;

in ivec2 aTileOffset;
in ivec2 aTileOrigin;
in uvec4 aMaskTexCoord0;
in ivec2 aCtrlBackdrop;
in int aPathIndex;
in int aColor;

out vec3 vMaskTexCoord0;
out vec2 vColorTexCoord0;
out vec4 vBaseColor;
out float vTileCtrl;
out vec4 vFilterParams0;
out vec4 vFilterParams1;
out vec4 vFilterParams2;
out vec4 vFilterParams3;
out vec4 vFilterParams4;
out float vCtrl;

void main(){
    vec2 tileOrigin = vec2(aTileOrigin), tileOffset = vec2(aTileOffset);
    vec2 position =(tileOrigin + tileOffset)* uTileSize;

    ivec4 zValue = ivec4(texture(uZBuffer,(tileOrigin + vec2(0.5))/ vec2(uZBufferSize))* 255.0);
    if(aPathIndex <(zValue . x |(zValue . y << 8)|(zValue . z << 16)|(zValue . w << 24))){
        gl_Position = vec4(0.0);
        return;
    }

    uvec2 maskTileCoord = uvec2(aMaskTexCoord0 . x, aMaskTexCoord0 . y + 256u * aMaskTexCoord0 . z);
    vec2 maskTexCoord0 =(vec2(maskTileCoord)+ tileOffset)* uTileSize;
    if(aCtrlBackdrop . y == 0 && aMaskTexCoord0 . w != 0u){
        gl_Position = vec4(0.0);
        return;
    }

    int ctrl;
    computeTileVaryings(position,
                        aColor,
                        uTextureMetadata,
                        uTextureMetadataSize,
                        vColorTexCoord0,
                        vBaseColor,
                        vFilterParams0,
                        vFilterParams1,
                        vFilterParams2,
                        vFilterParams3,
                        vFilterParams4,
                        ctrl);

    vTileCtrl = float(aCtrlBackdrop . x);
    vCtrl = float(ctrl);
    vMaskTexCoord0 = vec3(maskTexCoord0, float(aCtrlBackdrop . y));
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

