//! A sketch may save `object_groups`: curves made one object, each with an id, a name and its
//! curves' ids. An imported SVG or DXF is one group now, and `sketch.group` makes more. Older
//! designs need no transformation: they have no groups. The last release warns about the newer
//! format number and opens the design, ignoring the groups: the curves are there, each on its own.
