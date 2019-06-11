TARGET_DIR?=../resources/shaders

EMPTY=

SHADERS=\
	debug_solid.fs.glsl \
	debug_solid.vs.glsl \
	debug_texture.fs.glsl \
	debug_texture.vs.glsl \
	demo_ground.fs.glsl \
	demo_ground.vs.glsl \
	fill.fs.glsl \
	fill.vs.glsl \
	post.fs.glsl \
	post.vs.glsl \
	reproject.fs.glsl \
	reproject.vs.glsl \
	stencil.fs.glsl \
	stencil.vs.glsl \
	tile_alpha.fs.glsl \
	tile_alpha_monochrome.vs.glsl \
	tile_alpha_multicolor.vs.glsl \
	tile_solid.fs.glsl \
	tile_solid_monochrome.vs.glsl \
	tile_solid_multicolor.vs.glsl \
	$(EMPTY)

INCLUDES=\
	post_convolve.inc.glsl \
	tile_alpha_vertex.inc.glsl \
	tile_multicolor.inc.glsl \
	post_gamma_correct.inc.glsl \
	tile_monochrome.inc.glsl \
	tile_solid_vertex.inc.glsl \
	$(EMPTY)

OUT=\
	$(SHADERS:%=$(TARGET_DIR)/gl3/%) \
	$(SHADERS:%.glsl=$(TARGET_DIR)/metal/%.metal) \
	$(EMPTY)

GLSL_VERSION=330
GLSLANGFLAGS=--auto-map-locations -I.

SPIRVCROSSFLAGS=--msl --msl-version 020100 --msl-argument-buffers

SED_ARGS=-e "s/\#version 330/\#version \{\{version\}\}/" -e "s/\#line.*$$//"

all:	$(OUT)

.PHONY: clean

clean:
	rm -f $(OUT)

$(TARGET_DIR)/spirv/%.fs.spv:	%.fs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/spirv && glslangValidator $(GLSLANGFLAGS) -G$(GLSL_VERSION) -S frag -o $@ $<

$(TARGET_DIR)/gl3/%.fs.glsl:	%.fs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl3 && glslangValidator $(GLSLANGFLAGS) -S frag -E $< | sed $(SED_ARGS) > $@

$(TARGET_DIR)/spirv/%.vs.spv:	%.vs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/spirv && glslangValidator $(GLSLANGFLAGS) -G$(GLSL_VERSION) -S vert -o $@ $<

$(TARGET_DIR)/gl3/%.vs.glsl:	%.vs.glsl $(INCLUDES)
	mkdir -p $(TARGET_DIR)/gl3 && glslangValidator $(GLSLANGFLAGS) -S vert -E $< | sed $(SED_ARGS) > $@

$(TARGET_DIR)/metal/%.metal:	$(TARGET_DIR)/spirv/%.spv
	mkdir -p $(TARGET_DIR)/metal && spirv-cross $(SPIRVCROSSFLAGS) --output $@ $<
