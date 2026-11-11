//! The size a card may have on a chain's canvas: the one place the daemon and the window read
//! the limits from (the window reads them from the API document, as the defaults of `SizeLimits`).

use serde::{Deserialize, Serialize};
use toknila_core::Size;

/// The narrowest a card may be made, in canvas pixels.
pub const CARD_MIN_W: i32 = 196;
/// The widest a card may be made.
pub const CARD_MAX_W: i32 = 480;
/// The tallest a card may be made.
pub const CARD_MAX_H: i32 = 320;

fn min_w() -> i32 {
    CARD_MIN_W
}
fn max_w() -> i32 {
    CARD_MAX_W
}
fn max_h() -> i32 {
    CARD_MAX_H
}

/// The limits of a card's size by hand, as the API document states them.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeLimits {
    /// Narrowest width.
    #[cfg_attr(feature = "openapi", schema(default = min_w))]
    pub min_w: i32,
    /// Widest width.
    #[cfg_attr(feature = "openapi", schema(default = max_w))]
    pub max_w: i32,
    /// Tallest height.
    #[cfg_attr(feature = "openapi", schema(default = max_h))]
    pub max_h: i32,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            min_w: CARD_MIN_W,
            max_w: CARD_MAX_W,
            max_h: CARD_MAX_H,
        }
    }
}

/// Brings `size` into the limits (196 to 480 wide, at most 320 tall; never below 1 tall).
pub fn clamp_size(size: Size) -> Size {
    Size {
        w: size.w.clamp(CARD_MIN_W, CARD_MAX_W),
        h: size.h.clamp(1, CARD_MAX_H),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_outside_the_limits_is_brought_in() {
        let s = |w, h| Size { w, h };
        assert_eq!(clamp_size(s(10, -5)), s(196, 1));
        assert_eq!(clamp_size(s(9000, 9000)), s(480, 320));
        assert_eq!(clamp_size(s(300, 120)), s(300, 120));
        assert_eq!(
            SizeLimits::default(),
            SizeLimits {
                min_w: 196,
                max_w: 480,
                max_h: 320
            }
        );
    }
}
