//! A saved star may follow another (`follows` in the star's source: the followed star's link id, `flip`,
//! `turn` in degrees, `shift` and `line`, the id of the curve it was mirrored in): a mirror copy of a star is such a star, and it takes the shape of
//! the star it follows (points, radius, depth, skip, roundings, circles) and keeps its own place, which follows the mirror line when that moves.
//! Older designs have no such copies and need no transformation. The last release warns about the
//! newer format number, opens the design and reads the copy as a star of its own with the numbers it
//! was saved with, so it no longer follows.
