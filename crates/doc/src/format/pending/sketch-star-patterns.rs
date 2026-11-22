//! A sketch pattern may copy stars: the pattern saves the link ids of the stars it copies
//! (`stars`), and each of its instances the link ids of the stars made for that copy (`stars`, in
//! the same order). Each such copy is a star that follows its source (`follows`, with the turn and
//! shift of its place in the pattern), so it takes the source's shape and moves the pattern's way.
//! Older designs have no such patterns and need no transformation. The last release warns about the
//! newer format number, opens the design and reads each copy as a star of its own with the numbers
//! it was saved with; the pattern keeps its other copies but no longer adds or removes stars.
