#   Map tile definitions
#

class RegionBox :
    def __init__(self, pos, size) :
        """
        Usual new
        """
        assert(len(pos) == 2);
        assert(len(size) == 2);
        self.pos = pos
        self.size = size
        
        
    def union(self, other) :
        ##  Lower left and upper right.
        ll = [min(self.pos[0],other.pos[0]), 
                  min(self.pos[1],other.pos[1])];
        ur = [max(self.pos[0] + self.size[0], other.pos[0] + other.size[0]), 
                 max(self.pos[1] + self.size[1], other.pos[1] + other.size[1])];
        size = [ur[0] - ll[0], ur[1] - ll[1]];
        return RegionBox(ll, size)
        
    def overlaps(self, other) :
        """
        True if regions overlap
        """
        xnolap = self.pos[0] + self.size[0] <= other.pos[0] or self.pos[0] >= other.pos[0] + other.size[0]
        ynolap = self.pos[1] + self.size[1] <= other.pos[1] or self.pos[1] >= other.pos[1] + other.size[1]
        return not (xnolap or ynolap)
        
    def contains(self, other) : 
        """
        True if self contains other
        """
        xcontain = self.pos[0] <= other.pos[0] and self.pos[0] + self.size[0] >= other.pos[0] + other.size[0]
        ycontain = self.pos[1] <= other.pos[1] and self.pos[1] + self.size[1] >= other.pos[1] + other.size[1]
        return xcontain and ycontain
        
        
def test_region_box() :
    """
    Basic sanity check
    """
    boxa = RegionBox([100,100], [10,10])
    boxb = RegionBox([105,105], [10,10])
    boxc = RegionBox([110,110], [10,10])
    assert(boxa.overlaps(boxb))
    assert(not boxa.overlaps(boxc))
    combine = boxa.union(boxb)
    assert(combine.contains(boxa))
    assert(combine.contains(boxb))
    assert(not combine.contains(boxc))
    print("Test_region_box passed")
    
#   Unit test
if __name__ == "__main__" :
    test_region_box()
