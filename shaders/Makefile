TARGET_DIR?=../resources/shaders

EMPTY=

SHADERS=\
	blit.fs.glsl \
	blit.vs.glsl \
	clear.fs.glsl \
	clear.vs.glsl \
	debug_solid.fs.glsl \
	debug_solid.vs.glsl \
	debug_texture.fs.glsl \
	debug_texture.vs.glsl \
	demo_ground.fs.glsl \
	demo_ground.vs.glsl \
	fill.fs.glsl \
	fill.vs.glsl \
	reproject.fs.glsl \
	reproject.vs.glsl \
	stencil.fs.glsl \
	stencil.vs.glsl \
	tile.fs.glsl \
	tile.vs.glsl \
	tile_clip.fs.glsl \
	tile_clip.vs.glsl \
	tile_copy.fs.glsl \
	tile_copy.vs.glsl \
	$(EMPTY)

COMPUTE_SHADERS=\
	fill.cs.glsl \
	$(EMPTY)

INCLUDES=\
	fill.inc.glsl \
	$(EMPTY)

OUT=\
	$(SHADERS:%=$(TARGET_DIR)/gl3/%) \
	$(SHADERS:%=$(TARGET_DIR)/gl4/%) \
	$(SHADERS:%.glsl=$(TARGET_DIR)/metal/%.metal) \
	$(SHADERS:%.glsl=build/metal/%.spv) \
	$(COMPUTE_SHADERS:%=$(TARGET_DIR)/gl4/%) \
	$(COMPUTE_SHADERS:%.glsl=$(TARGET_DIR)/metal/%.metal) \
	$(COMPUTE_SHADERS:%.glsl=build/metal/%.spv) \
	$(EMPTY)

GLSL_3_VERSION=330
GLSL_4_VERSION=430
GLSLANGFLAGS=--auto-map-locations -I.
GLSLANGFLAGS_METAL=$(GLSLANGFLAGS) -DPF_ORIGIN_UPPER_LEFT=1

SPIRVCROSS?=spirv-cross
SPIRVCROSSFLAGS=--msl --msl-version 020100

GLSL_VERSION_HEADER="\#version {{version}}"
HEADER="// Automatically generated from files in pathfinder/shaders/. Do not edit!"

GLSL_SED_ARGS=-e "s/\#version .*//" -e "s/\#line.*$$//"

all:	$(OUT)

.PHONY: clean

clean:
	rm -f $(OUT)

build/metal/%.fs.spv:	%.fs.glsl $(INCLUDES)
	mkdir -p build/metal && glslangValidator $(GLSLANGFLAGS_METAL) -G$(GLSL_VERSION) -S frag -o $@ $<

$(TARGET_DIR)/gl3/%.fs.glsl:	%.fs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl3 && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( glslangValidator $(GLSLANGFLAGS) -S frag -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

$(TARGET_DIR)/gl4/%.fs.glsl:	%.fs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl4 && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( glslangValidator $(GLSLANGFLAGS) -S frag -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

build/metal/%.vs.spv:	%.vs.glsl $(INCLUDES)
	mkdir -p build/metal && glslangValidator $(GLSLANGFLAGS_METAL) -G$(GLSL_VERSION) -S vert -o $@ $<

$(TARGET_DIR)/gl3/%.vs.glsl:	%.vs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl3 && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( glslangValidator $(GLSLANGFLAGS) -S vert -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

$(TARGET_DIR)/gl4/%.vs.glsl:	%.vs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl3 && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( glslangValidator $(GLSLANGFLAGS) -S vert -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

build/metal/%.cs.spv:	%.cs.glsl $(INCLUDES)
	mkdir -p build/metal && glslangValidator $(GLSLANGFLAGS_METAL) -G$(GLSL_COMPUTE_VERSION) -S comp -o $@ $<

$(TARGET_DIR)/gl4/%.cs.glsl:	%.cs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl4 && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( glslangValidator $(GLSLANGFLAGS) -S vert -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

$(TARGET_DIR)/metal/%.metal:	build/metal/%.spv
	mkdir -p $(TARGET_DIR)/metal && echo $(HEADER) > $@ && ( $(SPIRVCROSS) $(SPIRVCROSSFLAGS) $< >> $@ ) || ( rm $@ && exit 1 )
