//! An Emboss or Deboss may save more profiles (`more_profiles`: a sketch and its profiles each, from
//! the same sketch or others), more picked faces after `face` (`more_faces`), a shift of its
//! profiles (`dx`, `dy` along the sketch's axes and `angle` about their centre, #19616) and `flip`
//! (raised or sunk toward the sketch's back). Older designs need no transformation: they have none
//! of these fields and build as before. The last release warns about the newer format number,
//! opens the design and ignores the fields it does not know, so such a feature builds from its
//! first sketch and face only, unshifted and unflipped.
