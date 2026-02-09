# Terrain uploading plan

John Nagle
August, 2025

## Introduction
The general idea is to make terrain visible out to the horizon.
At the user level, the plan is documented here:

https://www.animats.com/sharpview/technotes/impostor.html

This note is about how all those terrain objects get created
and uploaded.

## Terrain height measurement

This is version 2 of the Big World plan. Version 1 was 
using flat squares from the SL map. Worked, so we had
a proof of concept. Version 2 is terrain sculpts for
each region, with larger multi-region ones for 4, 16, 64 regions, etc.

Height information is collected by an LSL script, making llGround
calls on 4-meter centers.
The script formats the info into one byte of elevation data for each
height measurement, and packages that up as a huge hex string.
That's assembled into a JSON object, which is then sent to our server
on "animats.info".

At the server end, not much happens at this stage. Just storage.

SQL setup:

Database "terrain".
Users: 

"terrainuploader" - can append, only.
"terrainreader" - can read, only.
"terrainmgr" - can do everything
(More users to come) 

Tables:

raw_terrain_heights: 
- the raw JSON uploaded by the script above, with some additional fields - scale, water height, etc.
- avatar UUID and timestamp of initial uploader. 

  Later attempts to upload are checked to see if the new terrain data is different. If it is,
  the new data replaces the old data. If it is the same, the "confirmer" avatar UUID and timestamp
  are added.


SQL looks roughly like:

    CREATE TABLE 
      json content
      southwest corrner of region
      uploader_id, upload_timestamp.
      confirmer_id, confirmer_timestamp
      
   unique on location.
   Entry goodness:
      - uploader and confirmer ID are different.
      - uploader is on trusted list (?)
      - 30 days with no changes.
      - This may be overkill.
      
Database created but only user is "terrainmgr"

2025-09-01

Prelminary version of terrain logging is working.
LSL script is working. 
Table raw_terrain_heights has been filled in for the Blake Sea area by carrying around a scripted object and flying over.
Current updater will not change terrain if flown over again and the terrain is different. That needs to be fixed.

Next steps:

- Generate sculpts from above data. Same algorithm as old Python code.
  - Test before a big upload.
- Generate multi-region data for lower LODs. 
  - Edge problems. For now, if no data for a region, treat it as water when building multi-region data.
  - Have to generate the multi-region images too. 
    - What to do when we have multi-region image info but not elevation?
      - Generate the images, although they'll be underwater in some areas.
      - Better than having two sets of images for large regions.
- Add sculpt support in Sharpview.
- Overfly Blake Sea and make video.

2025-09-02

- Converted all region positions to meters. [DONE]
- Downscaling elevations.
  - Create target array, 2D of f32. Initialize to 0
  - Get an iterator over a SELECT of the region of interest. Retrieve relevant squares.
  - Apply samples to target array.
  - Get Z bounds for target array. Compute scaling and offset. Generate elevation image?
  
- Uploading needs to require a PUT operation. 

- The sculpt maker program needs to create a local directory of named images.
  - This will be a command line program.
  
  Transitive closure:
  
  - SELECT grid, x, y, size_x, size_y ORDER BY grid, x, y;  
  - Maintain an ordered list of sets of ??? (x, y) with size_x, size_y.
  - Process sequentially.
    - On control break of grid, done with grid, clear and restart.
    - On control break of x, start new column.
    
    - Within a column:
      - If Y item touches preceding Y item
        - Merge into that items set
      - If next Y item touches an item in the ordered list
        - Merge into that item's set.
      - Otherwise start new set
    
    - Start new column
      - Anything not wide enough to reach new column, done.
      
    ***MORE***
   
2025-09-07

   Can detect all touching blocks. Lots of (a touches b) items.
   a and b have an ordering. Reverse pair so that they all
   have the same order.
   Lowest entry in set has a set of all the others. All others
   have a link back to the lowest entry.
   Merging requires changing a lot of links.
   Is there another way?
   
2025-09-10

   New plan:
   - VizGroup has a set of weak backlinks to the LiveBlock items.
   - On VizGroup merge,
     - All linked LiveBlocks get their VizGroup updated.
       - But this creates a circular borrow problem at the LiveBlock level.
       - Have to distinguish between the LiveBlock whose VizGroup isn't changing (the survivor,
         currently borrowed) and othe LiveBlock items. The others get their VizGroup changed.
         They're not currently borrowed. 
       - Also at that time, purge dead weak links from the set of weak backlinks.
       
    This has tricky borrow plumbing.
    
2025-09-19

   Above all works.
   Must change database schema - keep samples_x and samples_y because big blob is a 2D array.
   - Set all values to 64, but should be 65. FIX.
   
2025-09-22

   Uploaded data for Blake Sea seems to be wrong. Recheck upload side.
   
2025-09-23

   Generated a valid impostor sculpt. Now need to generate JSON file.
   Also need to re-capture all the elevs; the data is bad.
  
2025-09-26

   Data collection looks good. Manually generated sculpts look good.
   Need some kind of authentication for the capture script.
   
   Next steps: generate a JSON file for the viewer automatically.
   - Need a database first.
   - Where do we get UUIDs?
     - Upload script - finds all sculpt images and adds their UUIDs.
   - Generate large-area merged sculpts.
   
   - Got sculpt display into Sharpview.
   - Sculpts are being generated with X and Y exchanged.
   
2025-09-29

   Sculpt impostors working.
   Need to generate sculpts with max, instead of min, so rocks
   in water are above the water line. [DONE]
   
2025-10-02

   Wrote LSL script to look at uploaded sculpt textures and collect
   info to send to server. Wrote SQL for region_impostor table.
   Need to write Rust server to handle uploads and queries.
   - Queries needed "what viz_group is this region in", 
     "give me all JSON for this viz_group", and "give me all JSON".
   - Upload needs some thought.
     - Need authentication for upload.
     
2025-10-04

   Beginning of impostor upload code.
   Find out why "Siren's Isle" came through as "Sirens".
   Parse enums like this: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=3c43bd9041eb567dbf974c8519ef10d7
   
2025-10-12

   Doing read from database for impostor data.
   - Fix length of uuid field in "regionimpostors" in database.
   - "MySQL" statement generator only allows up to 12 fields, and I have 18. Bleah.
   
2025-11-19

   Recovering from walking pneumonia.
   Downloadimpostors is working.
   Starting on uploadimpostors, where an LSL script tells the database what's been uploaded.
   - Sculpts or meshes are self-identifying from their names
   - Textures. Need to work on naming convention.    
   
2025-11-20

   Map tile plan:
   - New SQL table.
   - Upload all UUIDs from Grid Survey, once.
   - Future changes involve actually uploading a tile. 
   
2025-11-21
   Looks like there's no non-grid survey way to get map tiles in SL.
   So, have to get them from the map server.
   For SL, we can get those for any zoom level. For OS, not sure.
   
2025-11-25

   Give textures a name similar to that of the terrain sculpt/mesh.
   Different prefix.
   Consider adding a hash suffix to the file name to distinguish versions.
   Textures and sculpt/mesh have the same suffix. 
   - They're always replaced together.
   
   Do we still need another database table?
   
2025-12-04

   No, don't need another table, just a filename field in region_impostors to
   tell us when we don't need to make a new one.
   
   Single generation is working. Next, hierarchical generation.
   - Generate all singles. 
   - Work upwards - 4x, 16x, etc. until largest one covers the extents of the viz group.
   - Cache completed regions.
   - Every 2x rows, do 2x regions, iterating across row.
     - Every 4x rows, do 4x regions, etc. This keeps regions needed in cache without storing entire map in memory.
     
2025-12-16

   Hierarchy stuff underway. Harder than expected.
   Two modes:
   - No hierarchy, variable sized region.
   - Hierarchy, uniform sized regions.
     - Need non-uniform size detector.
     - Existing test case is not valid for hierarchy.
     
2025-12-25
   Merry Xmas!
   LOD loop control is all wrong.
   Fixes:
    - set fn to set Land, Water, with check for Unknown
    - one loop - type Loop, but has self.working_lod as a loop counter.
    - Two state variables: self.working_lod, self.progress_made
    - New advance statuses: None, Data(item), Progress.
      - Hold working LOD on Data(item) and Progress (i.e. water). Set progress_made.
      - Advance working_lod if None and progress_made.
      - Reset working_lod to 0 if None and !progress_made.
      - If hit lowest LOD and progress_made not set, done.
      - ***CHECK THIS***
      
2025-12-21
    All coded, not working.
    Out of order entry from vizgroup causing crash.
    - Items returned in order 2, 3, 4, 5, 0.
    - Does vizgroup guarantee order? It's supposed to.
    - Add in-sequence check.
    - Added sort. Fixed.
    All LODs are not advancing in sync.
    - "Tested cell of invalid row: x: 100, row 0: 0, row 1: -100" should never happen.
    
 2026-01-02
 
    Not going well.
    Need to rethink shift/scan loop.
    - We can only scan if aligned with the previous LOD.
      - Aligned means lower Y for working LOD is same as lower Y of above LOD.
      - Each LOD is not aligned half the time, because each LOD moves in 2x the jumps of previous.
    - LOD must shift column if the higher (smaller number) LOD is ahead of us.
      - Ahead means Y of previous LOD by our height or more.
      - Don't shift if shift will not produce alignment.
        - Needs a loop to shift more than once?
    - Scan LODs from highest to lowest and scan if aligned.
    - Stop scan when a non-aligned LOD is hit.
    - How does completion work at end of input?
      - Must get all LODs into alignment somehow.
      - Need to generate dummy empty rows of water to run out the end?
    - Distinguish between column is finished and doing a shift.
      - ?
      
2026-01-02

    What's going on here.
    
    We have an ordered list of all the regions of interest. 
    It's sparse; there are holes. That's LOD 0. We want to generate lower
    level LOD lists of regions of interest. LOD 1 is a grid
    of four region cells. LOD 2 is a grid of 16 region cells,
    and so forth.
    
    The obvious way to do this would be to first create a large 2D array of
    region records for LOD 0. Then, pass over it in units of 4 cells,
    and create a quarter-sized 2D array of 4-cell blocks. If there's a cell
    at LOD 0 for any of the cells of a 4-cell block, create the 4-cell
    block as "Land". All unused cells become "Water".
    
    Repeat this for each lower (higher numbered) LOD until there is one giant
    block that covers everything.
    
    This takes too much memory. So we try to do it sequentially. We only need
    the last two columns of each LOD to create the next lower LOD.
    So we only need to store two columns of history.
    
    Main loop within iterator:
    
    - Return first entry on output list, if any.
    
    - Read a new LOD 0 region. 
      Add it to the output list, because higher (smaller LOD #) regions must precede
      lower ones. The code that actually builds the region maps requires that.
      
    - If the current column is not the right one for the new region:
      - Call column finished on the current column for LOD 0. Marks as water out to end.
      - We just finished a column of LOD 0, and we have to tell the
        lower LODs about that. 
        - Iterate over LOD from 1..N.
        - If LOD is not aligned, stop.
        - If aligned, scan and finish LOD, perhaps recording a new region.
        
      - After the lower LODs have been updated, we shift the active
        two columns to align for the new LOD 0 region. 
        (What if there's a jump of more than one column in the LOD 0 region list?
        Advance the column, and create a column of all water, until we align again.
        Each time we do this, we do shift/align on all lower LODs. So all LODs are in sync.
        
      - Now we are aligned on columns. 
         
    - Mark the new region in column 0 of the two columns stored.
      - Iterate over LOD from 1..N
      - If LOD is not aligned, stop.
      - If aligned, scan LOD, perhaps recording a new region.
         
    Finally, return first entry on output list. (There must be one until EOF).
      
      Lower LOD entries have three functions - scan, column finished, and align/shift.
      - Scan: proceed across the row, looking for four cells valid above, and marking any skipped
        cells as water. If we find a new valid cell, return its region info.
        - Stop Y scan on first success, or return array of regions?
        Caller will add such regions to the output list.
        
      - Column finished: mark the remaining cells in the row as water. No return data.
      
      - Align/Shift: The hard one. Shift the stored two columns until they align with the next
        higher LOD. This is touchy and prone to off by one errors.
        - Input is the current left (?) X of LOD 0.
        - Anything shifted must already be water or land.
      
    - EOF on input:
      - Finish the current row, and proceed as usual for that.
      - Add dummy rows until all lower LODs report they are done (how?)
      
Complete, but correct?

2026-01-06

    Plugging away on above. Notes:
    - Fill first columns of LOD with water to get started.
    - Runout is scan then align until lowest LOD item is emitted.
    - Much further along.
    
2026-01-07

    Almost ready to test:
    TODO:
    - Expand rows or colums as needed to make the work area square.
    - Write EOF runout code.
    
2026-01-08
    LOD alignment problems.
    Each LOD must be aligned on a power of 2 times the x and y sizes. They're not.
    
2016-01-09
    Alignment OK. Runout has problems.
    
2016-01-12
    Scan logic is wrong. We must scan lower LODs each time we insert land or water.
    Not doing that.
    
    The only scanning takes place at end of a column.
    
    So, after mark_lod_0, we need to do a scan of lod_n. No shifting. If there's an
    aligned lower LOD, mark it, possibly emitting a region as output.
    
    But when we're marking an odd (not aligned with a lower LOD) column, we can't
    mark lower LODs. So we're just accumulating marks.
    
    The effect is that we're going to build up a backlog of lower LOD tiles that can't
    go out until the entire column has been marked and we're starting on the next column.
    So this will have more work in progress than was anticipated. A whole row, perhaps a few thousand tiles.
    Could be several gigabytes of RAM, since the images are in there.
    
    Given that, do we want to do all the lower LOD work at the end of each LOD 0 column? Simpler.
    
2026-01-13

    Beginning to work. Generates sculpt textures, but they have a blue line running vertically through them.
    - Fixed that.
    - Scale is off by a factor of 2. combine/halve is wrong.
      - No, using wrong image
      
2026-01-14
   Getting reasonable sculpts but Z offsets for individual regions are being combined wrong.

2026-01-17
   Good results. Now add checking for duplicates in database, and add hash value to region_impostors.
   Then need to update LSL scripts, and add a function to create a JSON file in the format the viewers
   currently understand.
   - Hash is 16 hex digits. Add to region_impostors as height_hash CHAR(16). Add to faces_json as "hash" for each texture item.
     - Viewer never uses these fields.
     - Plan ahead for more detailed models.
     
2026-01-22
    Unified names region_loc, etc. between upload and download side.
    Need to add versioning for upload, but put that as a parameter on the URL in future.
    - In progress: more checking for duplicates in generate
    - Next: recheck upload.
    - Todo: download format with version info?
    
2026-01-24
    Design problem: vizgroup numbers need to be persistent.
    - Do they need to be part of texture names? Probably, so uploadimpostor can work easily.
    - Generateterrain needs to track this.
    - Check impostor database and build correspondence between old vizgroup number and new one.
      - New/old mapping. For each vizgroup, search impostor database for a matching object and map new vizgroup to stored vizgroup.
        - Majority voting, so if permanent vizgroups need to change, the majority wins?
        - A new region which joins two vizgroups means a big change to the impostor database.
          - A change that doesn't involve uploading new impostors. 
          - Argues against holding vizgroup data in terrain object names.
          - Terrain generator may have to update existing entries.
            - Not a big deal.
          - What happens when vizgroups within a larger impostor set change.
            - It's possible to have multiple multi-region impostors for same regions but different vizgroups. 
              - region_impostors unique index now has a problem. 
         - Picking new vizgroup values
            - They're already sorted by size, so they probably won't change in ways that affect large numbers of impostors.
            - Assume we have full impostor upload data in raw_terrain_heights. So vizgroup values are complete.
            - We could just replace the old ones with the new ones and it would work, although viewers might be out of sync until they relog.
              - Change as little as possible.
              - New = old, unless we have to change.
                - Old is found by looking up first region of a new vizgroup in region_impostors. 
                  - Worst case we have to do a join to find matching single-region impostors.
          - Orphaned multi-region impostors are a problem.

2026-01-25
    Vizgroup numbers
    - New field in region_impostors: uniqueness_vizgroup, which is None for 1-region tiles, valid for others, and part of unique index.
      - This prevents having a 1-region tile (LOD 0) in more than one vizgroup.
    - After computing vizgroups, but before region order, generate table of vizgroup translations. Reads region_impostors but does not write it.
    - Generate new items with new vizgroups. Vizgroup becomes another field in filename.
    - Impostor upload in uploadimpostor sets vizgroup and uniqueness_vizgroup.
    - Garbage collection.
      - At end of uploadimpstors, do a garbage collection to delete entries in region_impostors which represent a tile of LOD>0 and 
        for which no tile at LOD 0 has that vizgroup.
        - Can this be done with one SQL statement?
    Region name was removed in favor of hash. But viewer uses names. 
    - Embedding name in filename has potential length problem. Names are limited to 63 bytes.
      - We can get the name from terrain_heights if we really need it. Null for now.
    Need base texture hash for each face. 
    - Problem. How do we get that from LSL? Upload does not know this.
      - We don't have face info at SL viewer upload time. Rethink.
        - Have to read in all the geometry and texture entries, then assemble them into the region_impostors table.
          - Just read them into temporary tables and do a join. Let SQL do the work.

2026-01-26
    Next design problem - need to do uploads in batches.
    - What's the limit on how many textures a prim can store as content?
    - Generator needs to generate lots of folders.
      - Limit on how much is number of records we can send in one HTTP request to uploadimpostors.
    - Each folder must contain all the textures needed for any object in it.
      - So the terrain generator must generate many small folders.
        - Maybe 50 meshes/sculpts, plus their textures.
        - Actual limit is about 10KB of filenames.
    - OK, suppose we have one big persistent table of textures. Then, when we see a sculpt of mesh, associate the textures and go.
      - Table tile_textures added.
            grid 
            region_loc_x region_loc_y 
            region_size_x region_size_y
            texture_index (always 0 for sculpts)
            lod
            viz_group
            uuid
            hash
            timestamp

      - Table cleanup/garbage collection?
        - Not essential for now. Unique on x, y, size_x, size_y, lod, viz_group, new replaces old. Defunct regions remain.
      - Too many manual uploads?
        - Workflow: upload one folder, put in prim. Run script in prim. Repeat 500 times.
        
2026-01-27
    tile uploader is running for textures, but some hash values are not 16 characters.
    - Lead zero problem.
    - Name length limitation in SL: 63 chars. But description is same as name, but limited to 127 chars.
    
2026-01-28
   Changed hash to 8 chars. Now it fits and works.
   Next, upload sculpt info and generate region_impostors database.
   - Forgot about emissive textures. Add support for that.
     - New record, RE, for emissive textures.
     - Add enum to tile_textures.
     - Emissive doesn't do anything yet, but put it in anyway.
     
2026-01-29
    Rename tile_textures to tile_assets
    Add enum for sculpt, mesh, base texture, emissive_texture.
    Put sculpt textures in tile_assets file.
    Add asset name string to tile_assets table.
    Unduplication of assets and vizgroup renaming works off tile_assets file.
    Move parsing of filename to server side.
    
    Close to getting useful output.
    But sql and database now out of sync. LSL uploader out of sync. 
    
2026-01-30
    Back in sync.
    Optimizations to put in:
    - When generating tiles, check if exact duplicate on filename, including hash.
      If so, no need to generate again.
    - Vizgroup numbering
      - Generated numbers from the current pass are always used.
      - If a generated tile matches on everything but vizgroup,
        - Do not generate a new tile.
        - Generator adds a new entry for it in tile_asset with new visgroup number but old filename.
          - Add an "original vizgroup" field, default NULL, for tracking.
        - How does garbage collection work?
       - Efficient find first free vizgroup number with SQL?
     
    Generated region impostor database for the first time.
    TODO:
    - Do we need hash values? Probably not, remove from table after checking.
    - Pass creator info through to region_impostors table.
    - faces_json not getting set.
    - Optimization stuff.
    
2026-01-31
    Add grid to tile_assets table index as UNIQUE INDEX (grid, asset_name) [DONE]
    Need to add sculpts to tile_assets table so unduplication will find them.
        
2026-02-05
    Much confusion about avoiding re-upload of the same content when viz_group changes.
    Architecture is wrong.
    Need to:
    - Run generateterrain to generate upload list.
    - Upload all files generated to asset server.
    - Tell servers about uploaded files.
    - Only when all files have been uploaded, generate new region_impostors table.
    
    To do this, must:
    - Keep table of viz_group info from generateterrain. 
      - What's in that table?
        - It's a new version of the region_impostors table. Starts empty.
        - Many UUIDs are not filled in yet. 
          - Missing UUIDs are inserted as uploadimpostors runs.
          - When inserting UUIDs, try to find an existing tile_asset
            which matches on x,y,sx,sy,grid,hash but not vizgroup.
          - If no find, have to generate a new tile.
        - Use the viz groups from the generateterrain run.    
       - When all UUIDs are filled in, it becomes the new region_impostors table.     
    - How to know when all uploads are complete.
      - If all UUIDs are filled in, we're ready to replace the entire region_impostors table.
        - Atomic SQL operation.
        - Do we need a generation number or timestamp so that viewer can tell if table changed?
     - Workflow
       - Replacement happens automatically when all UUIDs are filled in.
       - Replacement is done by uploadimpostors.
       - Need error list.
   Think on this overnight. May have missed something.
   
2026-02-08
   Going well.
   Need, for sculpts, the base texture hash and optional UUID.
   - We have hash. Just need to look up by hash in tile_assets.
   - Asset type needs to be added to unique index.
   - Change asset type to enum.
   For meshes, it's not clear where the textures come from yet.
    

      
