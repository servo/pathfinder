// partitionfinder/partitionfinder.h

#ifndef PARTITIONFINDER_H
#define PARTITIONFINDER_H

#include <stdint.h>

#define PF_ANTIALIASING_MODE_MSAA   0
#define PF_ANTIALIASING_MODE_LEVIEN 1

#ifdef __cplusplus
extern "C" {
#endif

typedef uint8_t pf_antialiasing_mode_t;

typedef uint16_t pf_float16_t;

struct pf_point2d_f32 {
    float x, y;
};

typedef struct pf_point2d_f32 pf_point2d_f32_t;

struct pf_matrix2d_f32 {
    float m00, m01, m02;
    float m10, m11, m12;
};

typedef struct pf_matrix2d_f32 pf_matrix2d_f32_t;

struct pf_vertex {
    uint32_t prev_endpoint_index;
    uint32_t next_endpoint_index;
    float time;
    uint32_t padding;
};

typedef struct pf_vertex pf_vertex_t;

struct pf_edge_instance {
    uint32_t prev_endpoint_index;
    uint32_t next_endpoint_index;
    float prev_time;
    float next_time;
};

typedef struct pf_edge_instance pf_edge_instance_t;

struct pf_quad_tess_levels {
    pf_float16_t outer[4];
    pf_float16_t inner[2];
};

typedef struct pf_quad_tess_levels pf_quad_tess_levels_t;

struct pf_b_quad {
    uint32_t upper_prev_endpoint, upper_next_endpoint;
    uint32_t lower_prev_endpoint, lower_next_endpoint;
    float upper_left_time, upper_right_time;
    float lower_left_time, lower_right_time;
};

typedef struct pf_b_quad pf_b_quad_t;

struct pf_endpoint {
    pf_point2d_f32_t position;
    uint32_t control_points_index;
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

struct pf_tessellator;

typedef struct pf_tessellator pf_tessellator_t;

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
                              uint32_t first_subpath_index,
                              uint32_t last_subpath_index);

const pf_b_quad_t *pf_partitioner_b_quads(pf_partitioner_t *partitioner,
                                                uint32_t *out_b_quad_count);

pf_tessellator_t *pf_tessellator_new(const pf_endpoint_t *endpoints,
                                     uint32_t endpoint_count,
                                     const pf_point2d_f32_t *control_points,
                                     uint32_t control_point_index,
                                     const pf_b_quad_t *b_quads,
                                     uint32_t b_quad_count,
                                     pf_antialiasing_mode_t antialiasing_mode);

void pf_tessellator_destroy(pf_tessellator_t *tessellator);

void pf_tessellator_compute_hull(pf_tessellator_t *tessellator, const pf_matrix2d_f32_t *transform);

void pf_tessellator_compute_domain(pf_tessellator_t *tessellator);

const pf_quad_tess_levels_t *pf_tessellator_tess_levels(const pf_tessellator_t *tessellator,
                                                        uint32_t *out_tess_levels_count);

const pf_vertex_t *pf_tessellator_vertices(const pf_tessellator_t *tessellator,
                                           uint32_t *out_vertex_count);

const uint32_t *pf_tessellator_msaa_indices(const pf_tessellator_t *tessellator,
                                            uint32_t *out_msaa_index_count);

const pf_edge_instance_t *pf_tessellator_edge_instances(const pf_tessellator_t *tessellator,
                                                        uint32_t *out_edge_instance_count);

uint32_t pf_init_env_logger();

#ifdef __cplusplus
}
#endif

#endif
