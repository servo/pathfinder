TARGET_DIR?=../resources/shaders

EMPTY=

SHADERS=\
	d3d9/fill.fs.glsl \
	d3d9/fill.vs.glsl \
	d3d9/tile.fs.glsl \
	d3d9/tile.vs.glsl \
	d3d9/tile_clip_combine.fs.glsl \
	d3d9/tile_clip_combine.vs.glsl \
	d3d9/tile_clip_copy.fs.glsl \
	d3d9/tile_clip_copy.vs.glsl \
	d3d9/tile_copy.fs.glsl \
	d3d9/tile_copy.vs.glsl \
	debug/solid.fs.glsl \
	debug/solid.vs.glsl \
	debug/texture.fs.glsl \
	debug/texture.vs.glsl \
	blit.fs.glsl \
	blit.vs.glsl \
	clear.fs.glsl \
	clear.vs.glsl \
	demo_ground.fs.glsl \
	demo_ground.vs.glsl \
	reproject.fs.glsl \
	reproject.vs.glsl \
	stencil.fs.glsl \
	stencil.vs.glsl \
	$(EMPTY)

COMPUTE_SHADERS=\
	d3d11/bin.cs.glsl \
	d3d11/bound.cs.glsl \
	d3d11/dice.cs.glsl \
	d3d11/fill.cs.glsl \
	d3d11/propagate.cs.glsl \
	d3d11/sort.cs.glsl \
	d3d11/tile.cs.glsl \
	$(EMPTY)

INCLUDES=\
	d3d11/fill_compute.inc.glsl \
	fill_area.inc.glsl \
	tile_fragment.inc.glsl \
	tile_vertex.inc.glsl \
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

GLSLANG?=glslangValidator
GLSLANGFLAGS=--auto-map-locations -I.
GLSLANGFLAGS_METAL=$(GLSLANGFLAGS) -DPF_ORIGIN_UPPER_LEFT=1

SPIRVCROSS?=spirv-cross
SPIRVCROSSFLAGS=--msl --msl-version 020100

GLSL_VERSION_HEADER="\#version {{version}}"
HEADER="// Automatically generated from files in pathfinder/shaders/. Do not edit!"

GLSL_SED_ARGS=-e "s/\#version .*//" -e "s/\#line.*$$//"

GLSL_SHADER_TYPE.fs=frag
GLSL_SHADER_TYPE.vs=vert
GLSL_SHADER_TYPE.cs=comp

all:	$(OUT)

.PHONY: clean

clean:
	rm -f $(OUT)

build/metal/%.spv:	%.glsl $(INCLUDES)
	mkdir -p $(dir $@) && $(GLSLANG) $(GLSLANGFLAGS_METAL) -G$(GLSL_VERSION) -S $(GLSL_SHADER_TYPE$(suffix $(basename $(notdir $<)))) -o $@ $<

$(TARGET_DIR)/gl3/%.glsl:	%.glsl $(INCLUDES)
	mkdir -p $(dir $@) && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( $(GLSLANG) $(GLSLANGFLAGS) -S $(GLSL_SHADER_TYPE$(suffix $(basename $(notdir $<)))) -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

$(TARGET_DIR)/gl4/%.glsl:	%.glsl $(INCLUDES)
	mkdir -p $(dir $@) && echo $(GLSL_VERSION_HEADER) > $@ && echo $(HEADER) >> $@ && ( $(GLSLANG) $(GLSLANGFLAGS) -S $(GLSL_SHADER_TYPE$(suffix $(basename $(notdir $<)))) -E $< | sed $(GLSL_SED_ARGS) >> $@ ) || ( rm $@ && exit 1 )

$(TARGET_DIR)/metal/%.metal:   build/metal/%.spv
	mkdir -p $(dir $@) && echo $(HEADER) > $@ && ( $(SPIRVCROSS) $(SPIRVCROSSFLAGS) $< >> $@ ) || ( rm $@ && exit 1 )
