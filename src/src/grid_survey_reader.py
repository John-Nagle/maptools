#   Terrain and map picture UUIDs from Tyche's Grid Survey
#   June, 2025
#   Animats
#   License: LGPL.
#
from urllib.request import urlopen

QUERY_URL = "http://api.gridsurvey.com/simquery.php?xy=%i,%i"
'''
Fetch info for one region
'''
def fetch_region_info(coords) : 
    url = QUERY_URL % ((coords[0], coords[1]))
    print("Fetching %s" % url)
    with urlopen(url) as response :
        data = response.read().decode('utf-8')  # Decode bytes to string
        print(data)
        items = data.splitlines()
        for item in data.splitlines() :
            kv = item.split(" ",1)
            print(kv)
        
  
  
'''
Test 1 - fetch one known URL
'''
def test1() :
    coords = [1130, 1046]
    fetch_region_info(coords)


if __name__ == "__main__" :
    test1()
