#include <landscape.h>
#include <lumin/node/RootNode.h>
#include <lumin/node/QuadNode.h>
#include <lumin/resource/PlanarResource.h>
#include <lumin/ui/node/UiPanel.h>
#include <lumin/ui/Cursor.h>
#include <lumin/input/Raycast.h>
#include <lumin/event/RayCastEventData.h>
#include <ml_logging.h>
#include <scenes.h>
#include <PrismSceneManager.h>

int main(int argc, char **argv)
{
  ML_LOG(Debug, "PathfinderDemo Starting.");
  PathfinderDemo myApp;
  return myApp.run();
}

const char* QUAD_NAMES[1] = {
  "quad1"
};

const char* PANEL_NAMES[1] = {
  "uiPanel1"
};

PathfinderDemo::PathfinderDemo() {
  ML_LOG(Debug, "PathfinderDemo Constructor.");

  // Place your constructor implementation here.
  svg_filecount_ = magicleap_pathfinder_svg_filecount();
  svg_filenames_ = magicleap_pathfinder_svg_filenames();
}

PathfinderDemo::~PathfinderDemo() {
  ML_LOG(Debug, "PathfinderDemo Destructor.");

  // Place your destructor implementation here.
}

const glm::vec3 PathfinderDemo::getInitialPrismSize() const {
  return glm::vec3(0.4f, 0.4f, 0.4f);
}

void PathfinderDemo::createInitialPrism() {
  prism_ = requestNewPrism(getInitialPrismSize());
  if (!prism_) {
    ML_LOG(Error, "PathfinderDemo Error creating default prism.");
    abort();
  }
  prismSceneManager_ = new PrismSceneManager(prism_);
}

int PathfinderDemo::init() {

  ML_LOG(Debug, "PathfinderDemo Initializing.");

  createInitialPrism();
  lumin::ui::Cursor::SetEnabled(prism_, false);
  spawnInitialScenes();

  // Place your initialization here.
  if (checkPrivilege(lumin::PrivilegeId::kControllerPose) != lumin::PrivilegeResult::kGranted) {
    ML_LOG(Error, "Pathfinder Failed to get controller access");
    abort();
    return 1;
  }

  
  // Get the root node of the prism
  lumin::RootNode* root_node = prism_->getRootNode();
  if (!root_node) {
    ML_LOG(Error, "Pathfinder Failed to get root node");
    abort();
    return 1;
  }

  // Get the quad
  lumin::QuadNode* quad_node = lumin::QuadNode::CastFrom(prism_->findNode(QUAD_NAMES[0], root_node));
  if (!quad_node) {
    ML_LOG(Error, "Pathfinder Failed to get quad node");
    abort();
    return 1;
  }

  // Create the EGL surface for it to draw to
  lumin::ResourceIDType plane_id = prism_->createPlanarEGLResourceId();
  if (!plane_id) {
    ML_LOG(Error, "Pathfinder Failed to create EGL resource");
    abort();
    return 1;
  }
  lumin::PlanarResource* plane = static_cast<lumin::PlanarResource*>(prism_->getResource(plane_id));
  if (!plane) {
    ML_LOG(Error, "Pathfinder Failed to get plane");
    abort();
    return 1;
  }
  quad_node->setRenderResource(plane_id);

  return 0;
}

int PathfinderDemo::deInit() {
  ML_LOG(Debug, "PathfinderDemo Deinitializing.");

  // Place your deinitialization here.

  return 0;
}

void PathfinderDemo::spawnInitialScenes() {

  // Iterate over all the exported scenes
  for (auto& exportedSceneEntry : scenes::externalScenes ) {

    // If this scene was marked to be instanced at app initialization, do it
    const SceneDescriptor &sd = exportedSceneEntry.second;
    if (sd.getInitiallySpawned()) {
      lumin::Node* const spawnedRoot = prismSceneManager_->spawn(sd);
      if (spawnedRoot) {
        if (!prism_->getRootNode()->addChild(spawnedRoot)) {
          ML_LOG(Error, "PathfinderDemo Failed to add spawnedRoot to the prism root node");
          abort();
        }
      }
    }
  }
}

bool PathfinderDemo::updateLoop(float fDelta) {

  // Place your update here.

  // Return true for your app to continue running, false to terminate the app.
  return true;
}

bool PathfinderDemo::eventListener(lumin::ServerEvent* event) {
  // Place your event handling here.
  lumin::ServerEventType typ = event->getServerEventType();
  switch (typ) {
    case lumin::ServerEventType::kControlPose6DofInputEvent:
      requestWorldRayCast(getHeadposeWorldPosition(), getHeadposeWorldForwardVector(), 0);
      return false;
    case lumin::ServerEventType::kRayCastEvent: {
      lumin::RayCastEventData* raycast_event = static_cast<lumin::RayCastEventData*>(event);
      std::shared_ptr<lumin::RaycastResult> raycast_result = raycast_event->getHitData();
      switch (raycast_result->getType()) {
        case lumin::RaycastResultType::kQuadNode: {
	  std::shared_ptr<lumin::RaycastQuadNodeResult> quad_result = std::static_pointer_cast<lumin::RaycastQuadNodeResult>(raycast_result);
          focus_node = quad_result->getNodeId();
          return false;
	}
        default:
          focus_node = lumin::INVALID_NODE_ID;
          return false;
      }
    }
    default:
      return false;
  }
}

