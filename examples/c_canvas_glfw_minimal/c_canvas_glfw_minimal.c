// pathfinder/examples/c_canvas_glfw_minimal/c_canvas_glfw_minimal.c
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#include <GLFW/glfw3.h>
#include <pathfinder/pathfinder.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

static void HandleGLFWError(int errorCode, const char *description);
static const void *LoadGLFunction(const char *name, void *userdata);
static void HandleKeypress(GLFWwindow *window, int key, int scancode, int action, int mods);

int main(int argc, const char **argv) {
    // Set up GLFW.
    GLFWwindow *window;
    if (!glfwInit())
        return 1;
    glfwSetErrorCallback(HandleGLFWError);

    // Make sure we have at least a GL 3.0 context. Pathfinder requires this.
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 2);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GLFW_TRUE);

    // Open a window.
    window = glfwCreateWindow(640, 480, "Minimal canvas example (GLFW/C API)", NULL, NULL);

    // Make the OpenGL context current.
    glfwMakeContextCurrent(window);

    // Create a Pathfinder renderer.
    PFGLLoadWith(LoadGLFunction, NULL);
    PFGLDestFramebufferRef dest_framebuffer =
        PFGLDestFramebufferCreateFullWindow(&(PFVector2I){640, 480});
    PFGLRendererRef renderer = PFGLRendererCreate(PFGLDeviceCreate(PF_GL_VERSION_GL3, 0),
                                                  PFFilesystemResourceLoaderLocate(),
                                                  dest_framebuffer,
                                                  &(PFRendererOptions){
        (PFColorF){1.0, 1.0, 1.0, 1.0}, PF_RENDERER_OPTIONS_FLAGS_HAS_BACKGROUND_COLOR
    });

    // Make a canvas. We're going to draw a house.
    PFCanvasRef canvas = PFCanvasCreate(PFCanvasFontContextCreateWithSystemSource(),
                                        &(PFVector2F){640.0f, 480.0f});

    // Set line width.
    PFCanvasSetLineWidth(canvas, 10.0f);

    // Draw walls.
    PFCanvasStrokeRect(canvas, &(PFRectF){{75.0f, 140.0f}, {225.0f, 250.0f}});

    // Draw door.
    PFCanvasFillRect(canvas, &(PFRectF){{130.0f, 190.0f}, {170.0f, 250.0f}});

    // Draw roof.
    PFPathRef path = PFPathCreate();
    PFPathMoveTo(path, &(PFVector2F){50.0, 140.0});
    PFPathLineTo(path, &(PFVector2F){150.0, 60.0});
    PFPathLineTo(path, &(PFVector2F){250.0, 140.0});
    PFPathClosePath(path);
    PFCanvasStrokePath(canvas, path);

    // Render the canvas to screen.
    PFSceneRef scene = PFCanvasCreateScene(canvas);
    PFSceneProxyRef scene_proxy = PFSceneProxyCreateFromSceneAndRayonExecutor(scene);
    PFSceneProxyBuildAndRenderGL(scene_proxy, renderer, PFBuildOptionsCreate());
    glfwSwapBuffers(window);

    // Wait for a keypress.
    glfwSetKeyCallback(window, HandleKeypress);
    while (!glfwWindowShouldClose(window))
        glfwPollEvents();

    // Finish up.
    glfwTerminate();
    return 0;
}

static void HandleGLFWError(int errorCode, const char *description) {
    fprintf(stderr, "GLFW error: %s [%d]\n", description, errorCode);
    exit(1);
}

static void HandleKeypress(GLFWwindow *window, int key, int scancode, int action, int mods) {
    glfwSetWindowShouldClose(window, GLFW_TRUE);
}

static const void *LoadGLFunction(const char *name, void *userdata) {
    return glfwGetProcAddress(name);
}
