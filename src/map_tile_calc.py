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
            if (len(fields) == 2) :
                coords = [int(fields[0]), int(fields[1])]
                print("Enter size: ", end = "")
                size = int(input())
                tilecalc(coords, size)
            else :
                print("Invalid input: %s" % (coordsline))
        except (KeyError, ValueError) as e :
            print("Error: %s", e)
        except EOFError:
            break
    
"""
Basic tile math
"""
def tilecalc(coords, size) :
    x = coords[0]
    y = coords[1]
    if (x % 256 == 0) and (y % 256 == 0) :
        xtile = x / 256
        ytile = y / 256
        print("Tile X, Y: (%i,%i)" % (xtile, ytile))
        if (size % 256 == 0) :
            tilesize = size / 256
            lod = lodcalc(tilesize)
        else :
            print("Size not a multiple of 256")
    else :
        print ("x or y not a multiple of 256")
        
"""
Calculate LOD from size
"""
def lodcalc(size) :
    lod = math.floor(math.log2(size))
    print("LOD: %i" % lod)
    if (pow(2,lod) != size) :
        print("Size %i is not a power of 2" % (size))
    lod
    
    
if __name__ == "__main__" :
    calculator()

