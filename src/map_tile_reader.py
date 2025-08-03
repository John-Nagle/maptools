#   Map tile reader from SL map tiles
#   July, 2025
#   Animats
#   License: LGPL.
#
from urllib.request import urlopen
import json
from map_tile_defs import RegionBox

#   A map tile URL looks like this: https://secondlife-maps-cdn.akamaized.net/map-4-1000-1000-objects.jpg
MAP_FILENAME = "map-%i-%i-%i-objects.jpg"
MAP_TILE_URL = "https://secondlife-maps-cdn.akamaized.net/%s"

'''
Fetch one map tile. x and y are in units of regions.
    
LOD 1 is a one-region map tile here.
'''
def fetch_map_tile(lod, filename) :
    url = MAP_TILE_URL % (filename)
    print("Reading", url)
    with urlopen(url) as response :
        return response.read()
        
def construct_map_tile_filename(lod, coords) :
    return MAP_FILENAME % (lod, coords[0], coords[1])
   
''' 
Are these coords valid for this LOD?

Must be a multiple of a power of 2 of LOD-1
'''
def coords_valid_for_lod(lod, coords) :
    scale = pow(2,lod-1)
    return coords[0] % scale == 0 and coords[1] % scale == 0

'''
Scan a rectangular area of the map and output. Save images.

We only download tiles bigger than 256x256. because the
size 1 tiles are available by UUID already.
''' 
def download_map_rectangle_old(ll, ur, directory) :
    items = []
    for x in range(ll[0], ur[0]+1) : 
        for y in range(ll[1], ur[1]+1) :
            for lod in range(2,5) :
                try: 
                    coords = [x,y]
                    if not coords_valid_for_lod(lod, coords) :
                        continue
                    filename = construct_map_tile_filename(lod, coords)
                    img = fetch_map_tile(lod, filename)
                    path = directory + "/" + filename
                    with open(filename, "wb") as outfile :
                        outfile.write(img)
                        print(" Wrote " + filename)
                except KeyError as err :
                    print("Failed for [%i, %i]: %s" % (x, y, err))
    print("Done.")
    return items
    
def download_map_tile(box, lod, directory) :
    """
    Download relevant map tile
    """
    try: 
        coords = [int(box.pos[0] / 256), int(box.pos[1] / 256)]
        assert(coords_valid_for_lod(lod, coords))
        filename = construct_map_tile_filename(lod, coords)
        img = fetch_map_tile(lod, filename)
        path = directory + "/" + filename
        with open(filename, "wb") as outfile :
            outfile.write(img)
            print(" Wrote " + filename)
    except KeyError as err :
        print("Failed for [%i, %i]: %s" % (x, y, err))
    print("Done.")  
    
def download_map_rectangles(fullbox, directory) :
    for lod in range (2,8) :
        scaling = int(pow(2,lod-1))
        # We want all boxes at this LOD which overlap fullbox
        boxsize = scaling*256
        print("LOD: %i, scaling %i, boxsize: %i" % (lod, scaling, boxsize))
        #   Round down to get starting position that may be partly off edge of fullbox
        xstart = int(int(fullbox.pos[0]) / boxsize) * boxsize
        ystart = int(int(fullbox.pos[1]) / boxsize) * boxsize
        xend = int(fullbox.pos[0] + fullbox.size[0] + boxsize)
        yend = int(fullbox.pos[1] + fullbox.size[1] + boxsize)
        print(xstart, xend, boxsize) 
        for x in range (xstart, xend, boxsize) :
            for y in range (ystart, yend, boxsize) :
                thisbox = RegionBox([x, y], [boxsize, boxsize])
                download_map_tile(thisbox, lod, directory)
        
    


if __name__ == "__main__" :
    outdir = "tileimages"
    boxll = RegionBox([1130*256, 1046*256], [256, 256])
    boxur = RegionBox([1139*256, 1054*256], [256, 256])
    # encloses all
    fullbox = boxll.union(boxur)   
    download_map_rectangles(fullbox, outdir)
    ###### download_map_rectangle_old([1130, 1046], [1139, 1054], outdir) # Blake Sea
