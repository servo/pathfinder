project(
  'pathfinder-example',
  'c',
  version: '0.1.0'
)

dep_glfw = dependency('glfw3')
dep_pathfinder = dependency('pathfinder-0')

executable(
  'pathfinder-example',
  'src/main.c',
  dependencies: [dep_glfw, dep_pathfinder],
  install: true
)
