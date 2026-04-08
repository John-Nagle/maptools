//! rect.rs -- 2D rectangle utilities.
//!
//! Part of the Animats impostor system.
//!
//!     License: LGPL.
//!     Animats
//!     April, 2026.
//!

/// Rect - 2D rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Rect<T: PartialEq+PartialOrd> {
    //  Lower left
    ll: [T;2],
    //  Upper right
    ur: [T;2],
}

impl<T: PartialEq+PartialOrd> Rect<T> {
    /// Usual new
    pub fn new(ll: [T;2], ur: [T;2]) -> Self {
        assert!(ur[0] >= ll[0]);
        assert!(ur[1] >= ll[1]);
        Self { ll, ur }
    }
    
    /// True if overlaps. Not just touches.
    pub fn overlaps(&self, other: &Self) -> bool {
        let axis_overlap = |axis| other.ur[axis] > self.ll[axis] && other.ll[axis] < self.ur[axis];
        axis_overlap(0) && axis_overlap(1)
    }
    
    /// True if touches
    pub fn touches(&self, other: &Self) -> bool {
        let axis_overlap = |axis| other.ur[axis] >= self.ll[axis] && other.ll[axis] <= self.ur[axis];
        axis_overlap(0) && axis_overlap(1)
    }
    
    /// True if no area
    pub fn is_empty(&self) -> bool {
        self.ll[0] == self.ur[0] || self.ll[1] == self.ur[1]
    }
}

// Implement Display for Rect
impl<T: std::fmt::Display+std::cmp::PartialOrd> std::fmt::Display for Rect<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Write the formatted string into the formatter
        write!(f, "({}, {})-({}, {})", self.ll[0], self.ll[1], self.ur[0], self.ur[1])
    }
}

/// Useful Rect
pub type RectU32 = Rect::<u32>;

#[test]
fn test_rect() {
    let r0 = Rect::<u32>::new([10,10], [20,20]);
    let r1 = Rect::<u32>::new([20,20], [30,30]);
    let r2 = RectU32::new([21,20], [29,30]);
    assert!(!r0.is_empty());
    assert!(!r1.is_empty());
    assert!(r0.touches(&r1));
    assert!(!r0.overlaps(&r1));
    assert!(r1.overlaps(&r2));
    println!("r2: {}", r2);
}
