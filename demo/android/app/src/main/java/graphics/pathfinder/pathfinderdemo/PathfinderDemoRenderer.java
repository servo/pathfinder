package graphics.pathfinder.pathfinderdemo;

import android.content.res.AssetManager;
import android.opengl.GLSurfaceView;

import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

public class PathfinderDemoRenderer extends Object implements GLSurfaceView.Renderer {
    private AssetManager m_assetManager;

    private static native void init(PathfinderDemoResourceLoader resourceLoader);
    private static native void runOnce();

    public static native void pushMouseDownEvent(int x, int y);
    public static native void pushMouseDraggedEvent(int x, int y);

    static {
        System.loadLibrary("pathfinder_android_demo");
    }

    protected PathfinderDemoRenderer() {}

    PathfinderDemoRenderer(AssetManager assetManager) {
        super();
        m_assetManager = assetManager;
    }

    @Override
    public void onSurfaceCreated(GL10 gl, EGLConfig config) {
        init(new PathfinderDemoResourceLoader(m_assetManager));
    }

    @Override
    public void onSurfaceChanged(GL10 gl, int width, int height) {

    }

    @Override
    public void onDrawFrame(GL10 gl) {
        runOnce();
    }
}
