#   Terrain and map picture UUIDs from Tyche's Grid Survey
#   June, 2025
#   Animats
#   License: LGPL.
#
from urllib.request import urlopen
import json

QUERY_URL = "http://api.gridsurvey.com/simquery.php?xy=%i,%i"
QUERY_GRID = "aditi"    # only supported grid
REGION_SIZE = 256
MESH_UUID = "29ab2808-a708-9b7a-d44b-06f62d3d5e5b" # Our cube for flat impostors
'''
Fetch info for one region
'''
def fetch_region_info(coords) : 
    url = QUERY_URL % ((coords[0], coords[1]))
    print("Fetching %s" % url)
    fields = {}
    with urlopen(url) as response :
        data = response.read().decode('utf-8')  # Decode bytes to string
        print(data)
        items = data.splitlines()
        fields = {}
        for item in data.splitlines() :
            kv = item.split(" ",1)
            print(kv)
            fields[kv[0]] = kv[1]
        
    return fields
            
def build_impostor_struct(coords, fields) :
    impostor_data = {  
        "comment" : "Flat impostor for region '%s'" % fields["name"],
        "grid" : QUERY_GRID,
        "region_loc" : [coords[0]*REGION_SIZE, coords[1]*REGION_SIZE],
        "scale": [256.0, 256.0, 23.0],
        "impostor_lod" : 0,
        "estate_id" : 0,
        "mesh_uuid" : MESH_UUID,
        "elevation_offset" : 0.0,
        "water_height" : 20.0,
        "faces" : [
            { "base_texture_uuid": fields["objects_uuid"],
              "emissive_texture_uuid": None,
            }
         ],
         "terrain_uuid": fields["terrain_uuid"] # currently unused
    }
    return impostor_data
        
              
  
  
'''
Test 1 - fetch one known URL
'''
def test1() :
    coords = [1130, 1046]
    fields = fetch_region_info(coords)
    print("Fields: ",fields)
    impostor_data = build_impostor_struct(coords, fields)
    print("Impostor data", impostor_data)
    json_data = json.dumps(impostor_data, indent=4)
    print("JSON data:\n", json_data)
   
'''
Scan a rectangular area of the map and output. Return JSON
''' 
def scan_map_rectangle(ll, ur) :
    items = []
    for x in range(ll[0], ur[0]+1) : 
        for y in range(ll[1], ur[1]+1) :
            try: 
                coords = [x,y]
                fields = fetch_region_info(coords)
                impostor_data = build_impostor_struct(coords, fields)
                items.append(impostor_data)
            except KeyError as err :
                print("Failed for [%i, %i]: %s" % (x, y, err))
    print("Done.")
    return items
    


if __name__ == "__main__" :
    ####test1()
    jout = scan_map_rectangle([1130, 1046], [1139, 1054]) # Blake Sea
    outfile = "blakeseaimpostors.json"
    ######jout = scan_map_rectangle([1130, 1046], [1131, 1047])
    with open(outfile, "w") as file:
        file.write(json.dumps(jout, indent = 4))
