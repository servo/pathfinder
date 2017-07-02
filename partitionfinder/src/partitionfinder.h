// partitionfinder/partitionfinder.h

#ifndef PARTITIONFINDER_H
#define PARTITIONFINDER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

struct pf_point2d_f32 {
    float x, y;
};

typedef struct pf_point2d_f32 pf_point2d_f32_t;

struct pf_bezieroid {
    uint32_t upper_prev_endpoint, upper_next_endpoint;
    uint32_t lower_prev_endpoint, lower_next_endpoint;
    uint32_t upper_left_time, upper_right_time;
    uint32_t lower_left_time, lower_right_time;
};

typedef struct pf_bezieroid pf_bezieroid_t;

struct pf_endpoint {
    pf_point2d_f32_t position;
    uint32_t control_points_index;
    uint32_t subpath_index;
};

typedef struct pf_endpoint pf_endpoint_t;

struct pf_control_points {
    pf_point2d_f32_t point1, point2;
};

typedef struct pf_control_points pf_control_points_t;

struct pf_subpath {
    uint32_t first_endpoint_index, path_index;
};

typedef struct pf_subpath pf_subpath_t;

struct pf_partitioner;

typedef struct pf_partitioner pf_partitioner_t;

pf_partitioner_t *pf_partitioner_new(const pf_endpoint_t *endpoints,
                                     uint32_t endpoint_count,
                                     const pf_control_points_t *control_points,
                                     uint32_t control_points_count,
                                     const pf_subpath_t *subpaths,
                                     uint32_t subpath_count);

void pf_partitioner_destroy(pf_partitioner_t *partitioner);

void pf_partitioner_reset(const pf_endpoint_t *new_endpoints,
                          uint32_t new_endpoint_count,
                          const pf_control_points_t *new_control_points,
                          uint32_t new_control_points_count,
                          const pf_subpath_t *new_subpaths,
                          uint32_t new_subpath_count);

void pf_partitioner_partition(pf_partitioner_t *partitioner);

const pf_bezieroid_t *pf_partitioner_bezieroids(pf_partitioner_t *partitioner,
                                                uint32_t *out_bezieroid_count);

#ifdef __cplusplus
}
#endif

#endif
