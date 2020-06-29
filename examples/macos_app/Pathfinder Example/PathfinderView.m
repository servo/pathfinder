//
//  PathfinderView.m
//  Pathfinder Example
//
//  Created by Patrick Walton on 6/21/19.
//  Copyright © 2019 The Pathfinder Project Developers. All rights reserved.
//

#import <QuartzCore/QuartzCore.h>
#import "PathfinderView.h"
#import <Metal/Metal.h>
#include <math.h>

static CVReturn outputCallback(CVDisplayLinkRef displayLink,
                               const CVTimeStamp *now,
                               const CVTimeStamp *outputTime,
                               CVOptionFlags flagsIn,
                               CVOptionFlags *flagsOut,
                               void *userData) {
    [(__bridge PathfinderView *)userData _render];
    return kCVReturnSuccess;
}

@implementation PathfinderView

#define FONT_SIZE   256.0f

- (void)_render {
    [mRenderLock lock];

    CGSize size = mLayerSize;

    PFCanvasRef canvas = PFCanvasCreate(mFontContext, &(PFVector2F){size.width, size.height});
    PFFillStyleRef fillStyle =
    PFFillStyleCreateColor(&(PFColorU){0, 0, 0, 255});
    float scaleX = cosf((float)mFrameNumber * 0.02);
    PFTransform2F textTransform;
    textTransform.matrix = (PFMatrix2x2F){scaleX, 0.0, 0.0, 1.0};
    textTransform.vector = (PFVector2F){size.width * 0.5, size.height * 0.5};
    PFCanvasSetTransform(canvas, &textTransform);
    PFCanvasSetFillStyle(canvas, fillStyle);
    PFCanvasSetFontSize(canvas, FONT_SIZE);
    PFCanvasSetTextAlign(canvas, PF_TEXT_ALIGN_CENTER);
    PFCanvasFillText(canvas, "Pathfinder", 0, &(PFVector2F){0.0, FONT_SIZE * 0.5});
    PFFillStyleDestroy(fillStyle);

    PFSceneRef scene = PFCanvasCreateScene(canvas);
    PFSceneProxyRef sceneProxy =
        PFSceneProxyCreateFromSceneAndRayonExecutor(scene, PF_RENDERER_LEVEL_D3D11);

    PFTransform2F pfTransform;
    pfTransform.matrix.m00 = 1.0;
    pfTransform.matrix.m01 = 0.0;
    pfTransform.matrix.m10 = 0.0;
    pfTransform.matrix.m11 = 1.0;
    pfTransform.vector.x = 0.0;
    pfTransform.vector.y = 0.0;

    PFBuildOptionsRef buildOptions = PFBuildOptionsCreate();
    PFRenderTransformRef renderTransform = PFRenderTransformCreate2D(&pfTransform);
    PFBuildOptionsSetTransform(buildOptions, renderTransform);
    PFSceneProxyBuildAndRenderMetal(sceneProxy, mRenderer, buildOptions);

    PFMetalDeviceRef pfMetalDevice = PFMetalRendererGetDevice(mRenderer);
    PFMetalDevicePresentDrawable(pfMetalDevice, mCurrentDrawable);
    mCurrentDrawable = [mLayer nextDrawable];
    PFMetalDeviceSwapDrawable(pfMetalDevice, mCurrentDrawable);

    mFrameNumber++;

    [mRenderLock unlock];
}

- (void)_checkCVResult:(CVReturn)result {
    if (result != kCVReturnSuccess) {
        @throw [NSException exceptionWithName:@"CoreVideoCallFailed"
                                       reason:@"Core Video call failed"
                                     userInfo:nil];
    }
}

- (void)_initializeIfNecessary:(CAMetalLayer *)layer {
    if (mDevice != nil)
        return;

    mFrameNumber = 0;

    mDevice = MTLCreateSystemDefaultDevice();
    [layer setDevice:mDevice];
    [layer setContentsScale:[[self window] backingScaleFactor]];

    mRenderLock = [[NSLock alloc] init];
    mLayerSize = [self convertSizeToBacking:[layer bounds].size];
    mCurrentDrawable = [layer nextDrawable];
    mLayer = layer;

    PFMetalDeviceRef device = PFMetalDeviceCreateWithDrawable(mDevice, mCurrentDrawable);
    PFResourceLoaderRef resourceLoader = PFFilesystemResourceLoaderLocate();
    PFMetalDestFramebufferRef destFramebuffer =
    PFMetalDestFramebufferCreateFullWindow(&(PFVector2I){mLayerSize.width, mLayerSize.height});

    PFRendererMode rendererMode;
    rendererMode.level = PF_RENDERER_LEVEL_D3D11;
    PFRendererOptions rendererOptions;
    rendererOptions.background_color = (PFColorF){1.0, 1.0, 1.0, 1.0};
    rendererOptions.flags = PF_RENDERER_OPTIONS_FLAGS_HAS_BACKGROUND_COLOR;
    rendererOptions.dest = destFramebuffer;
    mRenderer = PFMetalRendererCreate(device,
                                      resourceLoader,
                                      &rendererMode,
                                      &rendererOptions);

    mFontContext = PFCanvasFontContextCreateWithSystemSource();

    mBuildOptions = PFBuildOptionsCreate();

    [self _checkCVResult:CVDisplayLinkCreateWithActiveCGDisplays(&mDisplayLink)];
    [self _checkCVResult:CVDisplayLinkSetOutputCallback(mDisplayLink,
                                                        outputCallback,
                                                        (__bridge void *_Nullable)(self))];
    [self _checkCVResult:CVDisplayLinkStart(mDisplayLink)];
}

- (CALayer *)makeBackingLayer {
    return [[CAMetalLayer alloc] init];
}

- (BOOL)wantsLayer {
    return YES;
}

- (BOOL)wantsUpdateLayer {
    return YES;
}

- (NSViewLayerContentsRedrawPolicy)layerContentsRedrawPolicy {
    return NSViewLayerContentsRedrawOnSetNeedsDisplay;
}

- (void)drawRect:(NSRect)dirtyRect {
    [self _initializeIfNecessary:(CAMetalLayer *)[self layer]];
}

- (void)displayLayer:(CALayer *)layer {
    [self _initializeIfNecessary:(CAMetalLayer *)layer];
}

- (void)awakeFromNib {
    [self _initializeIfNecessary:(CAMetalLayer *)[self layer]];
}

@end
