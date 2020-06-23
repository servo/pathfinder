#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform vec2 uFramebufferSize;

in ivec2 aTileOffset;
in int aDestTileIndex;
in int aDestBackdrop;
in int aSrcTileIndex;
in int aSrcBackdrop;

out vec2 vTexCoord0;
out float vBackdrop0;
out vec2 vTexCoord1;
out float vBackdrop1;

void main(){
    vec2 destPosition = vec2(ivec2(aDestTileIndex % 256, aDestTileIndex / 256)+ aTileOffset);
    vec2 srcPosition = vec2(ivec2(aSrcTileIndex % 256, aSrcTileIndex / 256)+ aTileOffset);
    destPosition *= vec2(16.0, 4.0)/ uFramebufferSize;
    srcPosition *= vec2(16.0, 4.0)/ uFramebufferSize;

    vTexCoord0 = destPosition;
    vTexCoord1 = srcPosition;

    vBackdrop0 = float(aDestBackdrop);
    vBackdrop1 = float(aSrcBackdrop);

    if(aDestTileIndex < 0)
        destPosition = vec2(0.0);




    gl_Position = vec4(mix(vec2(- 1.0), vec2(1.0), destPosition), 0.0, 1.0);
}

