//! A sketch group (`object_groups`) may save a `lock`: `protect` (the whole object may move, turn
//! and scale, its points may not be edited one by one) or `total` (nothing may change). A group
//! without one is free, as every older design's groups are. The last release warns about the newer
//! format number and opens the design, ignoring the lock: the group's curves are then free.
