//! A sketch's object group may save `angle`: how far (degrees) it has been turned, so its
//! Transform box and a later stretch follow the group's own width and height. Older designs need
//! no transformation: a group without it has not been turned (0). The last release warns about
//! the newer format number and opens the design, ignoring the angle: the box is back on the
//! sketch's axes.
