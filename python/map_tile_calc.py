#   Map tile definitions
#
import math
"""
Useful calculator for map tile math
"""
def calculator() :
    print("Map tile math calculator for Second Life map tiles.")
    while True:
        try:
            print("Enter x,y: ", end = "")
            coordsline = input()
            fields = coordsline.split(",")
            if (len(fields) != 2) :
                raise ValueError("Invalid input: %s" % (coordsline))
            coords = [int(fields[0]), int(fields[1])]
            print("Enter size: ", end = "")
            size = int(input())
            tilecalc(coords, size)
        except (KeyError, ValueError) as e :
            print("Error: %s" % e)
        except EOFError:
            print("")
            break
    
"""
Basic tile math
"""
def tilecalc(coords, size) :
    x = coords[0]
    y = coords[1]
    if (x % 256 != 0) or (y % 256 != 0) :
        raise ValueError("X or Y not a multiple of 256")
    xtile = x / 256
    ytile = y / 256
    print("Tile X, Y: (%i,%i)" % (xtile, ytile))
    if (size % 256 != 0) :
        raise ValueError("Size not a multiple of 256")
    tilesize = size / 256
    lod = lodcalc(tilesize)
    if (x % size == 0 and y % size == 0) :
        print("Tile is size-aligned.")
    else :
        xaligned = int(x/size)*size
        yaligned = int(y/size)*size
        print("Tile is not size-aligned. Aligned corner is at (%i,%i)." % (xaligned, yaligned))
        
"""
Calculate LOD from size
"""
def lodcalc(size) :
    lod = math.floor(math.log2(size))
    print("LOD: %i" % lod)
    if (pow(2,lod) != size) :
        raise ValueError("Size %i is not a power of 2" % (size))
    lod
    
    
if __name__ == "__main__" :
    calculator()

