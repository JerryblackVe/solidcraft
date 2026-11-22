//! A sketch group (`object_groups`) may save a `pivot`: the point the Transform box scales and
//! turns it about when that is not the middle of its box. A group without one uses the middle, as
//! every older design's groups do. The last release warns about the newer format number and opens
//! the design, ignoring the pivot: the box then turns and scales about its middle.
