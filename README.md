# Tools for manipulating Second Life map data.
Used for working with region impostors for the Sharpview viewer.

The goal is to create the illusion of infinite draw distance, by displaying a 3D terrain map in world beyond draw distance. 

# How this works.
Region impostors are ordinary sculpt or mesh objects with a texture. They're stored as SL/OS textures and meshes. 
Because they're displayed beyond draw distance, the sim servers don't know about them. There's a separate impostor
server which tracks their UUIDs and provides that information to the viewer.

## Impostor creation
### 

## Database

### Table raw_terrain_heights
This table contains raw terrain height information obtained by flying over
regions with an LSL script which queries terrain height at a large number of points in a grid pattern.
The script calls the **uploadterrain** service on the impostor to upload new height data. 
Only the flyover script can do this.
### Table region_impostors
This table is the index to the UUIDs of the region impostor assets. 
It doesn't contain any images or geometry, just index information.
Viewers access it via the **downloadimpostor** service on the impostor server. Anyone can do this.

## Impostor creation.
Impostor assets are created by the **generateterrain** batch job.
This reads the height data from the database and creates the necessary sculpt and texture files.
A hash of the height data is computed, and checked against a hash in the **region_impostors** table to see if the terrain has changed.
If it has, a new terrain sculpt or texture file is emitted.

The generateterrain job generates a folder of textures to be uploaded to the asset servers.
This is currently done manually, from a viewer, as one bulk upload. The newly uploaded
items are moved to a prim, along with an LSL script.
The LSL script is run, and updates the **region_impostors** table via the **uploadimpostors** service
to update the index used by viewers.

