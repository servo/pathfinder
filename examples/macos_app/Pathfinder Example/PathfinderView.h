//
//  PathfinderView.h
//  Pathfinder Example
//
//  Created by Patrick Walton on 6/21/19.
//  Copyright © 2019 The Pathfinder Project Developers. All rights reserved.
//

#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#include <pathfinder_c.h>

NS_ASSUME_NONNULL_BEGIN

@interface PathfinderView : NSView {
    id<MTLDevice> mDevice;
    PFMetalRendererRef mRenderer;
    PFCanvasFontContextRef mFontContext;
    PFBuildOptionsRef mBuildOptions;
    CVDisplayLinkRef mDisplayLink;
    int32_t mFrameNumber;
    CAMetalLayer *mLayer;
    CGSize mLayerSize;
    NSLock *mRenderLock;
    id<CAMetalDrawable> mCurrentDrawable;
}

- (void)_render;

@end

NS_ASSUME_NONNULL_END
