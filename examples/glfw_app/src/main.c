// pathfinder/c/examples/src/main.c
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#include <stdio.h>
#include <GLFW/glfw3.h>
#include <pathfinder.h>

static void error_callback(int a, const char* b)
{
    fprintf(stderr, "GLFW error 0x%x: %s\n", a, b);
}

static const void *gl_loader(const char *name, void *userdata)
{
    (void)userdata;
    return glfwGetProcAddress(name);
}

static PFRectF PFRectFNew(PFVector2F origin, PFVector2F size)
{
    PFRectF rect;
    rect.origin = origin;
    rect.lower_right.x = origin.x + size.x;
    rect.lower_right.y = origin.y + size.y;
    return rect;
}

int main(int argc, char *argv[])
{
    GLFWwindow* window;
    int width, height;

    // Set up GLFW.
    glfwSetErrorCallback(error_callback);
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        exit(EXIT_FAILURE);
    }

    // Make sure we have at least a GL 3.3 context. Pathfinder requires this.
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    // Open a window.
    window = glfwCreateWindow(640, 480, "Pathfinder", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to open GLFW window\n");
        glfwTerminate();
        exit(EXIT_FAILURE);
    }

    // Create the GL context, and make it current.
    glfwMakeContextCurrent(window);
    PFGLLoadWith(gl_loader, NULL);
    glfwSwapInterval(1);

    // TODO: allow resizing and redraw on that event.
    glfwGetFramebufferSize(window, &width, &height);

    // Create a Pathfinder renderer.
    PFGLDeviceRef device = PFGLDeviceCreate(PF_GL_VERSION_GL3, 0);
    PFResourceLoaderRef resources =  PFFilesystemResourceLoaderLocate();
    PFVector2I window_size = {width, height};
    PFGLDestFramebufferRef framebuffer = PFGLDestFramebufferCreateFullWindow(&window_size);
    PFRendererOptions options = {
        .flags = PF_RENDERER_OPTIONS_FLAGS_HAS_BACKGROUND_COLOR,
        .background_color = (PFColorF){1.0, 1.0, 1.0, 1.0},
    };
    PFGLRendererRef renderer = PFGLRendererCreate(device, resources, framebuffer, &options);

    // Make a canvas. We're going to draw a house.
    PFCanvasFontContextRef font_context = PFCanvasFontContextCreateWithSystemSource();
    PFVector2F context_size = {width, height};
    PFCanvasRef canvas = PFCanvasCreate(font_context, &context_size);

    // Set line width.
    PFCanvasSetLineWidth(canvas, 10.0);

    // Draw walls.
    PFRectF stroke_rect = PFRectFNew((PFVector2F){75.0, 140.0}, (PFVector2F){150.0, 110.0});
    PFCanvasStrokeRect(canvas, &stroke_rect);

    // Draw door.
    PFRectF fill_rect = PFRectFNew((PFVector2F){130.0, 190.0}, (PFVector2F){40.0, 60.0});
    PFCanvasFillRect(canvas, &fill_rect);

    // Draw roof.
    PFPathRef path = PFPathCreate();
    PFVector2F m = {50.0, 140.0};
    PFPathMoveTo(path, &m);
    PFVector2F l1 = {150.0, 60.0};
    PFPathLineTo(path, &l1);
    PFVector2F l2 = {250.0, 140.0};
    PFPathLineTo(path, &l2);
    PFPathClosePath(path);
    PFCanvasStrokePath(canvas, path);

    // Render the canvas to screen.
    PFSceneRef scene = PFCanvasCreateScene(canvas);
    PFSceneProxyRef scene_proxy = PFSceneProxyCreateFromSceneAndRayonExecutor(scene);
    PFBuildOptionsRef build_options = PFBuildOptionsCreate();
    PFSceneProxyBuildAndRenderGL(scene_proxy, renderer, build_options);
    glfwSwapBuffers(window);

    // Wait for a keypress.
    while (!glfwWindowShouldClose(window) && !glfwGetKey(window, GLFW_KEY_ESCAPE)) {
        glfwWaitEvents();
    }

    // Terminate GLFW and exit.
    glfwTerminate();
    return EXIT_SUCCESS;
}
