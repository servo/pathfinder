// pathfinder/partitioner/partitioner.h

#ifndef PATHFINDER_PARTITIONER_H
#define PATHFINDER_PARTITIONER_H

#include <limits.h>
#include <stdint.h>

#define PF_ANTIALIASING_MODE_MSAA   0
#define PF_ANTIALIASING_MODE_ECAA   1

#define PF_B_VERTEX_KIND_ENDPOINT_0             0
#define PF_B_VERTEX_KIND_ENDPOINT_1             1
#define PF_B_VERTEX_KIND_CONVEX_CONTROL_POINT   2
#define PF_B_VERTEX_KIND_CONCAVE_CONTROL_POINT  3

#ifdef __cplusplus
extern "C" {
#endif

typedef uint8_t pf_antialiasing_mode_t;

typedef uint16_t pf_float16_t;

typedef uint8_t pf_b_vertex_kind_t;

struct pf_point2d_f32 {
    float x, y;
};

typedef struct pf_point2d_f32 pf_point2d_f32_t;

struct pf_matrix2d_f32 {
    float m00, m01, m02;
    float m10, m11, m12;
};

typedef struct pf_matrix2d_f32 pf_matrix2d_f32_t;

struct pf_b_vertex {
    pf_point2d_f32_t position;
    uint32_t path_id;
    uint8_t tex_coord[2];
    pf_b_vertex_kind_t kind;
    uint8_t pad;
};

typedef struct pf_b_vertex pf_b_vertex_t;

struct pf_cover_indices {
    const uint32_t *interior_indices;
    uint32_t interior_indices_len;
    const uint32_t *curve_indices;
    uint32_t curve_indices_len;
};

typedef struct pf_cover_indices pf_cover_indices_t;

struct pf_line_indices {
    uint32_t left_vertex_index;
    uint32_t right_vertex_index;
};

typedef struct pf_line_indices pf_line_indices_t;

struct pf_curve_indices {
    uint32_t left_vertex_index;
    uint32_t right_vertex_index;
    uint32_t control_point_vertex_index;
    uint32_t pad;
};

typedef struct pf_curve_indices pf_curve_indices_t;

struct pf_edge_indices {
    const pf_line_indices_t *top_line_indices;
    uint32_t top_line_indices_len;
    const pf_curve_indices_t *top_curve_indices;
    uint32_t top_curve_indices_len;
    const pf_line_indices_t *bottom_line_indices;
    uint32_t bottom_line_indices_len;
    const pf_curve_indices_t *bottom_curve_indices;
    uint32_t bottom_curve_indices_len;
};

typedef struct pf_edge_indices pf_edge_indices_t;

struct pf_b_quad {
    uint32_t upper_left_vertex_index;
    uint32_t upper_right_vertex_index;
    uint32_t upper_control_point_vertex_index;
    uint32_t pad0;
    uint32_t lower_left_vertex_index;
    uint32_t lower_right_vertex_index;
    uint32_t lower_control_point_vertex_index;
    uint32_t pad1;
};

typedef struct pf_b_quad pf_b_quad_t;

struct pf_endpoint {
    pf_point2d_f32_t position;
    uint32_t control_point_index;
    uint32_t subpath_index;
};

typedef struct pf_endpoint pf_endpoint_t;

struct pf_subpath {
    uint32_t first_endpoint_index;
    uint32_t last_endpoint_index;
};

typedef struct pf_subpath pf_subpath_t;

struct pf_legalizer;

typedef struct pf_legalizer pf_legalizer_t;

struct pf_partitioner;

typedef struct pf_partitioner pf_partitioner_t;

pf_legalizer_t *pf_legalizer_new();

void pf_legalizer_destroy(pf_legalizer_t *legalizer);

const pf_endpoint_t *pf_legalizer_endpoints(const pf_legalizer_t *legalizer,
                                            uint32_t *out_endpoint_count);

const pf_point2d_f32_t *pf_legalizer_control_points(const pf_legalizer_t *legalizer,
                                                    uint32_t *out_control_point_count);

const pf_subpath_t *pf_legalizer_subpaths(const pf_legalizer_t *legalizer,
                                          uint32_t *out_subpaths_count);

void pf_legalizer_move_to(pf_legalizer_t *legalizer, const pf_point2d_f32_t *position);

void pf_legalizer_close_path(pf_legalizer_t *legalizer);

void pf_legalizer_line_to(pf_legalizer_t *legalizer, const pf_point2d_f32_t *endpoint);

void pf_legalizer_quadratic_curve_to(pf_legalizer_t *legalizer,
                                     const pf_point2d_f32_t *control_point,
                                     const pf_point2d_f32_t *endpoint);

void pf_legalizer_bezier_curve_to(pf_legalizer_t *legalizer,
                                  const pf_point2d_f32_t *point1,
                                  const pf_point2d_f32_t *point2,
                                  const pf_point2d_f32_t *endpoint);

pf_partitioner_t *pf_partitioner_new();

void pf_partitioner_destroy(pf_partitioner_t *partitioner);

void pf_partitioner_init(pf_partitioner_t *partitioner,
                         const pf_endpoint_t *endpoints,
                         uint32_t endpoint_count,
                         const pf_point2d_f32_t *control_points,
                         uint32_t control_point_count,
                         const pf_subpath_t *subpaths,
                         uint32_t subpath_count);

void pf_partitioner_partition(pf_partitioner_t *partitioner,
                              uint32_t path_id,
                              uint32_t first_subpath_index,
                              uint32_t last_subpath_index);

const pf_b_quad_t *pf_partitioner_b_quads(const pf_partitioner_t *partitioner,
                                          uint32_t *out_b_quad_count);

const pf_b_vertex_t *pf_partitioner_b_vertices(const pf_partitioner_t *partitioner,
                                               uint32_t *out_b_vertex_count);

const void pf_partitioner_cover_indices(const pf_partitioner_t *partitioner,
                                        pf_cover_indices_t *out_cover_indices);

const void pf_partitioner_edge_indices(const pf_partitioner_t *partitioner,
                                       pf_edge_indices_t *out_edge_indices);

uint32_t pf_init_env_logger();

#ifdef __cplusplus
}
#endif

#endif
