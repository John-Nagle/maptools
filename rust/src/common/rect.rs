//! rect.rs -- 2D rectangle utilities.
//!
//! Part of the Animats impostor system.
//!
//! License: LGPL.
//! Animats
//! April, 2026.
//!
use std::fmt::Debug;
use std::str::FromStr;
use regex::Regex;
use anyhow::{Error, anyhow};

/// Rect - 2D rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Rect<T: PartialEq+PartialOrd+Debug+std::str::FromStr> {
    //  Lower left
    pub ll: [T;2],
    //  Upper right
    pub ur: [T;2],
}

impl<T: PartialEq+PartialOrd+Debug+std::str::FromStr> Rect<T> {
    /// Usual new
    pub fn new(ll: [T;2], ur: [T;2]) -> Self {
        assert!(ur[0] >= ll[0]);
        assert!(ur[1] >= ll[1]);
        Self { ll, ur }
    }
    
    /// New with error handling
    pub fn try_new(ll: [T;2], ur: [T;2]) -> Result<Self, Error> {
        if ur[0] >= ll[0] || ur[1] >= ll[1] {
            Ok(Self { ll, ur })
        } else {
            Err(anyhow!("Corners of rectangle ({:?}- {:?}) are not ordered lower left, upper right", ll, ur))
        }
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
    
    /// Parse (x0,y0)-(x1,y1) forms.
    pub fn parse(s: &str) -> Result<Self, Error> where <T as FromStr>::Err: std::fmt::Debug {
        //  Parses (12,34)-(56.78) with whitespace.
        let re = Regex::new(r"\s*\(\s*([0-9]+)\s*\,\s*([0-9]+)\s*\)\s*\-\s*\(\s*([0-9]+)\s*\,\s*([0-9]+)\s*\)\s*").expect("Regex compile failed");
        let vals = re.captures(s).ok_or_else(|| anyhow!("Cannot parse rectangle bounds (n,n)-(n,n) from \"{}\"", s))?;
        println!("Vals: {:?}", vals);
        //  Unwrap is safe here because we just parsed digits with the regex.
        let llx: T = vals[1].parse().unwrap();
        let lly: T = vals[2].parse().unwrap();
        let urx: T = vals[3].parse().unwrap();
        let ury: T = vals[4].parse().unwrap();
        Self::try_new([llx, lly], [urx, ury])
    }
/*    
    /// Scale up by scale factor
    pub fn scale_by(&self, scale: u32) -> Self {
        Self {
            ll: [self.ll[0]*scale, self.ll[1]*scale],
            ur: [self.ur[0]*scale, self.ur[1]*scale],
        }
    }
*/
}

// Implement Display for Rect
impl<T: std::fmt::Display+std::cmp::PartialOrd+Debug+std::str::FromStr> std::fmt::Display for Rect<T> {
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
    //  Test with and without whitespace.
    let s0 = "(21,20)-(29,30)";
    let s1 = " ( 21 , 20 ) - ( 29 , 30 ) ";
    let parsed0 = Rect::<u32>:: parse(s0).expect("Parse s0 failed");
    let parsed1 = Rect::<u32>:: parse(s1).expect("Parse s1 failed");
    assert_eq!(parsed0, r2);
    assert_eq!(parsed1, r2);
}
