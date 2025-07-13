#   Map tile definitions
#
"""
Useful calculator for map tile math
"""
def calculator() :
    print("Map tile math calculator for Second Life map tiles.")
    while True:
        try:
            print("Enter x,y")
            line = input()
            fields = line.split(",")
            if (len(fields) == 2) :
                coords = [int(fields[0]), int(fields[1])]
                tilecalc(coords)
            else :
                print("Invalid input: %s" % (line))
        except (KeyError, ValueError) as e :
            print("Error: %s", e)
        except EOFError:
            break
    
"""
Basic tile math
"""
def tilecalc(coords) :
    x = coords[0]
    y = coords[1]
    if (x % 256 == 0) and (y % 256 == 0) :
        xtile = x / 256
        ytile = y / 256
        print("Tile X, Y: (%i,%i)" % (xtile, ytile))
    else :
        print ("Not a multiple of 256")
        
if __name__ == "__main__" :
    calculator()

