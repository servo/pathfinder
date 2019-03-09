#include <stdio.h>
#include <stdlib.h>

#include <chrono>
#include <cmath>

#ifndef EGL_EGLEXT_PROTOTYPES
#define EGL_EGLEXT_PROTOTYPES
#endif

#include <EGL/egl.h>
#include <EGL/eglext.h>

#ifndef GL_GLEXT_PROTOTYPES
#define GL_GLEXT_PROTOTYPES
#endif

#include <GLES3/gl3.h>
#include <GLES3/gl3ext.h>

#include <ml_graphics.h>
#include <ml_head_tracking.h>
#include <ml_perception.h>
#include <ml_lifecycle.h>
#include <ml_logging.h>
#include <ml_privileges.h>

// Entry point to the Rust code
extern "C" MLResult magicleap_pathfinder_demo(EGLDisplay egl_display, EGLContext egl_context);

// Constants
const char application_name[] = "com.mozilla.pathfinder.demo";

// Structures
struct application_context_t {
  int dummy_value;
};

struct graphics_context_t {

  EGLDisplay egl_display;
  EGLContext egl_context;

  GLuint framebuffer_id;
  GLuint vertex_shader_id;
  GLuint fragment_shader_id;
  GLuint program_id;

  graphics_context_t();
  ~graphics_context_t();

  void makeCurrent();
  void swapBuffers();
  void unmakeCurrent();
};

graphics_context_t::graphics_context_t() {
  egl_display = eglGetDisplay(EGL_DEFAULT_DISPLAY);

  EGLint major = 4;
  EGLint minor = 0;
  eglInitialize(egl_display, &major, &minor);
  eglBindAPI(EGL_OPENGL_API);

  EGLint config_attribs[] = {
    EGL_RED_SIZE, 8,
    EGL_GREEN_SIZE, 8,
    EGL_BLUE_SIZE, 8,
    EGL_ALPHA_SIZE, 0,
    EGL_DEPTH_SIZE, 24,
    EGL_STENCIL_SIZE, 8,
    EGL_NONE
  };
  EGLConfig egl_config = nullptr;
  EGLint config_size = 0;
  eglChooseConfig(egl_display, config_attribs, &egl_config, 1, &config_size);

  EGLint context_attribs[] = {
    EGL_CONTEXT_MAJOR_VERSION_KHR, 3,
    EGL_CONTEXT_MINOR_VERSION_KHR, 0,
    EGL_NONE
  };
  egl_context = eglCreateContext(egl_display, egl_config, EGL_NO_CONTEXT, context_attribs);
}

void graphics_context_t::makeCurrent() {
  eglMakeCurrent(egl_display, EGL_NO_SURFACE, EGL_NO_SURFACE, egl_context);
}

void graphics_context_t::unmakeCurrent() {
  eglMakeCurrent(NULL, EGL_NO_SURFACE, EGL_NO_SURFACE, NULL);
}

void graphics_context_t::swapBuffers() {
  // buffer swapping is implicit on device (MLGraphicsEndFrame)
}

graphics_context_t::~graphics_context_t() {
  eglDestroyContext(egl_display, egl_context);
  eglTerminate(egl_display);
}

// Callbacks
static void onStop(void* application_context)
{
  ((struct application_context_t*)application_context)->dummy_value = 0;
  ML_LOG(Info, "%s: On stop called.", application_name);
}

static void onPause(void* application_context)
{
  ((struct application_context_t*)application_context)->dummy_value = 1;
  ML_LOG(Info, "%s: On pause called.", application_name);
}

static void onResume(void* application_context)
{
  ((struct application_context_t*)application_context)->dummy_value = 2;
  ML_LOG(Info, "%s: On resume called.", application_name);
}

extern "C" void logMessage(MLLogLevel lvl, char* msg) {
  if (MLLoggingLogLevelIsEnabled(lvl)) {
    MLLoggingLog(lvl, ML_DEFAULT_LOG_TAG, msg);
  }
}

int main() {
  // set up host-specific graphics surface
  graphics_context_t graphics_context;

  // let system know our app has started
  MLLifecycleCallbacks lifecycle_callbacks = {};
  lifecycle_callbacks.on_stop = onStop;
  lifecycle_callbacks.on_pause = onPause;
  lifecycle_callbacks.on_resume = onResume;

  struct application_context_t application_context;
  application_context.dummy_value = 2;

  if (MLResult_Ok != MLLifecycleInit(&lifecycle_callbacks, (void*)&application_context)) {
    ML_LOG(Error, "%s: Failed to initialize lifecycle.", application_name);
    return -1;
  }

  if (MLResult_Ok != MLPrivilegesStartup()) {
    ML_LOG(Error, "%s: Failed to initialize privileges.", application_name);
    return -1;
  }
  if (MLPrivilegesRequestPrivilege(MLPrivilegeID_WorldReconstruction) != MLPrivilegesResult_Granted) {
    ML_LOG(Error, "Privilege %d denied.", MLPrivilegeID_WorldReconstruction);
    return -1;
  }
  if (MLPrivilegesRequestPrivilege(MLPrivilegeID_LowLatencyLightwear) != MLPrivilegesResult_Granted) {
    ML_LOG(Error, "Privilege %d denied.", MLPrivilegeID_LowLatencyLightwear);
    return -1;
  }
  
  // initialize perception system
  MLPerceptionSettings perception_settings;
  if (MLResult_Ok != MLPerceptionInitSettings(&perception_settings)) {
    ML_LOG(Error, "%s: Failed to initialize perception.", application_name);
  }

  if (MLResult_Ok != MLPerceptionStartup(&perception_settings)) {
    ML_LOG(Error, "%s: Failed to startup perception.", application_name);
    return -1;
  }

  // Run the demo!
  ML_LOG(Info, "%s: Begin demo.", application_name);
  MLResult status = magicleap_pathfinder_demo(graphics_context.egl_display, graphics_context.egl_context);
  ML_LOG(Info, "%s: End demo (%d).", application_name, status);

  // Shut down
  MLPerceptionShutdown();

  return 0;
}
