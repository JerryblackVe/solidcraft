//! An Emboss or Deboss on selected faces may save `distribute`: for each profile, in order, the
//! selected face it goes on, centred on that face's picked point and laid along its outward
//! normal (six letters on six faces). Older designs need no transformation: they have no such
//! field, and each profile goes onto the face it lies over, as before. The last release warns
//! about the newer format number, opens the design and ignores the field, so such a feature puts
//! each profile on the face it lies over, and refuses if one lies over none.
