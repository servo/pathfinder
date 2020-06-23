#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!














#extension GL_GOOGLE_include_directive : enable













precision highp float;





layout(local_size_x = 64)in;

uniform mat2 uTransform;
uniform vec2 uTranslation;
uniform int uPathCount;
uniform int uLastBatchSegmentIndex;
uniform int uMaxMicrolineCount;

layout(std430, binding = 0)buffer bComputeIndirectParams {




    restrict uint iComputeIndirectParams[];
};


layout(std430, binding = 1)buffer bDiceMetadata {




    restrict readonly uvec4 iDiceMetadata[];
};

layout(std430, binding = 2)buffer bPoints {
    restrict readonly vec2 iPoints[];
};

layout(std430, binding = 3)buffer bInputIndices {
    restrict readonly uvec2 iInputIndices[];
};

layout(std430, binding = 4)buffer bMicrolines {




    restrict uvec4 iMicrolines[];
};

void emitMicroline(vec4 microlineSegment, uint pathIndex, uint outputMicrolineIndex){
    if(outputMicrolineIndex >= uMaxMicrolineCount)
        return;

    ivec4 microlineSubpixels = ivec4(round(clamp(microlineSegment, - 32768.0, 32767.0)* 256.0));
    ivec4 microlinePixels = ivec4(floor(vec4(microlineSubpixels)/ 256.0));
    ivec4 microlineFractPixels = microlineSubpixels - microlinePixels * 256;

    iMicrolines[outputMicrolineIndex]=
        uvec4((uint(microlinePixels . x)& 0xffff)|(uint(microlinePixels . y)<< 16),
            (uint(microlinePixels . z)& 0xffff)|(uint(microlinePixels . w)<< 16),
            uint(microlineFractPixels . x)|(uint(microlineFractPixels . y)<< 8)|
            (uint(microlineFractPixels . z)<< 16)|(uint(microlineFractPixels . w)<< 24),
            pathIndex);
}


bool curveIsFlat(vec4 baseline, vec4 ctrl){
    vec4 uv = vec4(3.0)* ctrl - vec4(2.0)* baseline - baseline . zwxy;
    uv *= uv;
    uv = max(uv, uv . zwxy);
    return uv . x + uv . y <= 16.0 * 0.25 * 0.25;
}

void subdivideCurve(vec4 baseline,
                    vec4 ctrl,
                    float t,
                    out vec4 prevBaseline,
                    out vec4 prevCtrl,
                    out vec4 nextBaseline,
                    out vec4 nextCtrl){
    vec2 p0 = baseline . xy, p1 = ctrl . xy, p2 = ctrl . zw, p3 = baseline . zw;
    vec2 p0p1 = mix(p0, p1, t), p1p2 = mix(p1, p2, t), p2p3 = mix(p2, p3, t);
    vec2 p0p1p2 = mix(p0p1, p1p2, t), p1p2p3 = mix(p1p2, p2p3, t);
    vec2 p0p1p2p3 = mix(p0p1p2, p1p2p3, t);
    prevBaseline = vec4(p0, p0p1p2p3);
    prevCtrl = vec4(p0p1, p0p1p2);
    nextBaseline = vec4(p0p1p2p3, p3);
    nextCtrl = vec4(p1p2p3, p2p3);
}

vec2 sampleCurve(vec4 baseline, vec4 ctrl, float t){
    vec2 p0 = baseline . xy, p1 = ctrl . xy, p2 = ctrl . zw, p3 = baseline . zw;
    vec2 p0p1 = mix(p0, p1, t), p1p2 = mix(p1, p2, t), p2p3 = mix(p2, p3, t);
    vec2 p0p1p2 = mix(p0p1, p1p2, t), p1p2p3 = mix(p1p2, p2p3, t);
    return mix(p0p1p2, p1p2p3, t);
}

vec2 sampleLine(vec4 line, float t){
    return mix(line . xy, line . zw, t);
}

vec2 getPoint(uint pointIndex){
    return uTransform * iPoints[pointIndex]+ uTranslation;
}

void main(){
    uint batchSegmentIndex = gl_GlobalInvocationID . x;
    if(batchSegmentIndex >= uLastBatchSegmentIndex)
        return;


    uint lowPathIndex = 0, highPathIndex = uint(uPathCount);
    int iteration = 0;
    while(iteration < 1024 && lowPathIndex + 1 < highPathIndex){
        uint midPathIndex = lowPathIndex +(highPathIndex - lowPathIndex)/ 2;
        uint midBatchSegmentIndex = iDiceMetadata[midPathIndex]. z;
        if(batchSegmentIndex < midBatchSegmentIndex){
            highPathIndex = midPathIndex;
        } else {
            lowPathIndex = midPathIndex;
            if(batchSegmentIndex == midBatchSegmentIndex)
                break;
        }
        iteration ++;
    }

    uint batchPathIndex = lowPathIndex;
    uvec4 diceMetadata = iDiceMetadata[batchPathIndex];
    uint firstGlobalSegmentIndexInPath = diceMetadata . y;
    uint firstBatchSegmentIndexInPath = diceMetadata . z;
    uint globalSegmentIndex = batchSegmentIndex - firstBatchSegmentIndexInPath +
        firstGlobalSegmentIndexInPath;

    uvec2 inputIndices = iInputIndices[globalSegmentIndex];
    uint fromPointIndex = inputIndices . x, flagsPathIndex = inputIndices . y;

    uint toPointIndex = fromPointIndex;
    if((flagsPathIndex & 0x40000000u)!= 0u)
        toPointIndex += 3;
    else if((flagsPathIndex & 0x80000000u)!= 0u)
        toPointIndex += 2;
    else
        toPointIndex += 1;

    vec4 baseline = vec4(getPoint(fromPointIndex), getPoint(toPointIndex));





    vec4 ctrl = vec4(0.0);
    float segmentCountF;
    bool isCurve =(flagsPathIndex &(0x40000000u |
                                                                        0x80000000u))!= 0;
    if(isCurve){
        vec2 ctrl0 = getPoint(fromPointIndex + 1);
        if((flagsPathIndex & 0x80000000u)!= 0){
            vec2 ctrl0_2 = ctrl0 * vec2(2.0);
            ctrl =(baseline +(ctrl0 * vec2(2.0)). xyxy)* vec4(1.0 / 3.0);
        } else {
            ctrl = vec4(ctrl0, getPoint(fromPointIndex + 2));
        }
        vec2 bound = vec2(6.0)* max(abs(ctrl . zw - 2.0 * ctrl . xy + baseline . xy),
                                     abs(baseline . zw - 2.0 * ctrl . zw + ctrl . xy));
        segmentCountF = sqrt(length(bound)/(8.0 * 0.25));
    } else {
        segmentCountF = length(baseline . zw - baseline . xy)/ 16.0;
    }


    int segmentCount = max(int(ceil(segmentCountF)), 1);
    uint firstOutputMicrolineIndex =
        atomicAdd(iComputeIndirectParams[3],
                  segmentCount);

    float prevT = 0.0;
    vec2 prevPoint = baseline . xy;
    for(int segmentIndex = 0;segmentIndex < segmentCount;segmentIndex ++){
        float nextT = float(segmentIndex + 1)/ float(segmentCount);
        vec2 nextPoint;
        if(isCurve)
            nextPoint = sampleCurve(baseline, ctrl, nextT);
        else
            nextPoint = sampleLine(baseline, nextT);
        emitMicroline(vec4(prevPoint, nextPoint),
                      batchPathIndex,
                      firstOutputMicrolineIndex + segmentIndex);
        prevT = nextT;
        prevPoint = nextPoint;
    }
}

