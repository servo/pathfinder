// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Common functions for all accumulation operations.

// Determines the boundaries of the column we'll be traversing.
void get_location(__private uint *lColumn,
                  __private uint *lFirstRow,
                  __private uint *lLastRow,
                  uint4 kAtlasRect,
                  uint kAtlasShelfHeight) {
    uint atlasWidth = kAtlasRect.z - kAtlasRect.x, atlasHeight = kAtlasRect.w - kAtlasRect.y;
    uint shelfIndex = get_global_id(0) / atlasWidth;

    *lColumn = get_global_id(0) % atlasWidth;
    *lFirstRow = min(shelfIndex * kAtlasShelfHeight, atlasHeight);
    *lLastRow = min((shelfIndex + 1) * kAtlasShelfHeight, atlasHeight);
}

