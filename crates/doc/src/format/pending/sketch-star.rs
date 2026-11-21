//! A sketch may save a star: a link of kind `star` whose source (`type: star`) holds its centre,
//! number of points, outer radius, valley depth, skip, roundings, turn and whether its construction
//! circles are drawn; its outlines are rebuilt from those numbers. Older designs need no
//! transformation: they have no star. The last release warns about the newer format number, opens
//! the design and keeps the sketch's other curves, but cannot read a star's link.
