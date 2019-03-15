package graphics.pathfinder.pathfinderdemo;

import android.content.res.AssetManager;
import android.opengl.GLSurfaceView;

import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

public class PathfinderDemoRenderer extends Object implements GLSurfaceView.Renderer {
    private AssetManager mAssetManager;
    private boolean mInitialized;

    private static native void init(PathfinderDemoResourceLoader resourceLoader,
                                    int width,
                                    int height);
    private static native int prepareFrame();
    private static native void drawScene(int sceneIndex);
    private static native void finishDrawingFrame();

    public static native void pushWindowResizedEvent(int width, int height);
    public static native void pushMouseDownEvent(int x, int y);
    public static native void pushMouseDraggedEvent(int x, int y);
    public static native void pushLookEvent(float pitch, float yaw);

    static {
        System.loadLibrary("pathfinder_android_demo");
    }

    protected PathfinderDemoRenderer() {}

    PathfinderDemoRenderer(AssetManager assetManager) {
        super();
        mAssetManager = assetManager;
        mInitialized = false;
    }

    @Override
    public void onSurfaceCreated(GL10 gl, EGLConfig config) {
    }

    @Override
    public void onSurfaceChanged(GL10 gl, int width, int height) {
        if (!mInitialized) {
            init(new PathfinderDemoResourceLoader(mAssetManager), width, height);
            mInitialized = true;
        } else {
            pushWindowResizedEvent(width, height);
        }
    }

    @Override
    public void onDrawFrame(GL10 gl) {
        int sceneCount = prepareFrame();
        for (int sceneIndex = 0; sceneIndex < sceneCount; sceneIndex++)
            drawScene(sceneIndex);
        finishDrawingFrame();
    }
}
