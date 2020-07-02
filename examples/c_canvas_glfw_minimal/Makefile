OBJ_DIR?=build
TARGET_DIR?=build
SRC_DIR?=.
RUST_TARGET_DIR?=../../target
RUST_SRC_DIR?=../../c
RUSTFLAGS?=

CFLAGS?=-Wall -g $(shell pkg-config --cflags pathfinder_c glfw3)
LIBS?=$(shell pkg-config --libs pathfinder_c glfw3)
MKDIR?=mkdir -p
RM?=rm
CARGO?=cargo

UNAME=$(shell uname -s)
ifeq ($(UNAME),Darwin)
	# FIXME(pcwalton): Don't link against HarfBuzz!!
	LIBS+=-framework OpenGL -framework CoreFoundation -framework CoreGraphics -framework CoreText
	LIBS+=-framework Metal -lharfbuzz
else
	LIBS+=-lGL
endif

ifeq ($(DEBUG),)
	CFLAGS+=-O2
else
	CFLAGS+=-O0
endif

all:	$(TARGET_DIR)/c_canvas_glfw_minimal

.PHONY:	clean

$(TARGET_DIR)/c_canvas_glfw_minimal:	$(OBJ_DIR)/c_canvas_glfw_minimal.o
	$(MKDIR) $(TARGET_DIR) && $(CC) $(LDFLAGS) $(LIBS) -o $@ $<

$(OBJ_DIR)/%.o:	$(SRC_DIR)/%.c
	$(MKDIR) $(OBJ_DIR) && $(CC) -c $(CFLAGS) -o $@ $<

clean:
	$(RM) -rf $(TARGET_DIR) && $(RM) -rf $(OBJ_DIR)
