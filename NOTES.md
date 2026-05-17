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
   - Add comments but do not implement.
   - When to generate an initial_impostors entry during generation? When we have both tile and texture UUIDs, or only tile?
     - Every time. Fill in the UUIDs later.
     
2026-02-09
    DB Access issue. Generateterrain wants to write terrain.interim_impostors table but is not allowed to do so from desktop.
    - Run on server?
    - Do updates via another server side program?
    - Make database locally and then upload the whole thing?
    - Temporarily, use
       ./generateterrain -c ../../../keys/generate_credentials.txt -o /tmp/imgs  -g agni
      which has a database account allowing remote access.
      
      
2026-02-10
     Final update runs, but fails to find missing UUIDs. Check SQL in initialimpostors.
     
2026-02-13
     Impostors are updated both in initialimpostors and in uploadimpostors. 
     - It looks like that if the texture info changes but the sculpt info does not, the new texture is not used.
       - Correct. Uploaded assets were textures only, so only the tile_assets table was updated.
       - Need to update initial_impostors when a new texture asset comes in.
         - Moderate sized design problem. Do this when textures come in, or wait until final commit?
         - Do it at final commit. The missing textures are detected there now, so fix them there.
    
2026-02-16
    Crashing at let result = conn.exec_first(SQL_LOOK_UP_UUID, select_params);
    - Not an SQL error, a panic? [FIXED]
    Remove uniqueness_vizgroup. [DONE]
    Appears to work. Seemingly valid region_impostors table created.
    TODO:
    - Need to tighten up security. How to identify clients?
    - Fix scripts to wait for status on each HTTP request.
    BUG:
    - Hashing of textures is not, apparently, totally repeatable.

2026-02-17
    Image files look the same but change from run to run.
    Pixels change, but not by more than 2 values.
    cmp imgs2/RT0_288768_268288_1024_1024_30.92_0.00_2_0_20.00_4fbb66fa.png 
        imgs3/RT0_288768_268288_1024_1024_30.92_0.00_2_0_20.00_d272bde4.png
    imgs2/RT0_288768_268288_1024_1024_30.92_0.00_2_0_20.00_4fbb66fa.png imgs3/RT0_288768_268288_1024_1024_30.92_0.00_2_0_20.00_d272bde4.png differ: byte 37, line 3
john@Nagle-LTS:/tmp$ 

    SL tile shown is 1128-1048, size 1024, or tile
    
    https://secondlife-maps-cdn.akamaized.net/map-3-1128-1048-objects.jpg
    
2026-02-18
    Still trying to find source of nonrepeatability.
    - Can check and store AWS hash if available.
      - Can check that with a header only read. 
    - Don't need more than one set of image tiles.
      - Elevation map has zero elevation for tiles of other viz groups.
      - So they won't show.
      - Add a dirt layer at elevation 0.01 to hide them if desired.
      
2026-02-19
    Definitely an Akamai stale cache situation alternatly serving two different
    versions of the same file.
    - Need to store timestamps in the tile_asset table.
      - Save "last-modified" in creation_time.
        - Only if not in future, as a safety check?
      - If a new read has an earlier last-modified than the one in the database, use the existing tile.

2026-02-20
   Trying to get rid of viz_group in tile info. 
   - But uploadimpostor can create initial_impostor entries. 
     - Should it be allowed to do that? Or only update them?
     - They're created in generateterrain, lacking only the final UUID.
     - May not need update_impostor_info at all.
       - Dont. Commented out.
    - Got rid of viz_group in tile info.
    
    Working on timestamp fixes. Reading last_modified time OK.
    - Checking rules:
      - Where do we use last_modified time?
      - It really ought to be part of the filename of an asset, but it won't fit.
      - Put in a tile_asset entry with no uuid and a timestamp when generating the asset?
      - Design trouble, again.
      
2026-02-21
    New plan:
    - Allow NULL for UUID in tile_asset.
      - Means generateterrain wants asset but it's not uploaded yet.
    - At generate time, check tile assets for matching x,y, lod, type.
      - If match, and has UUID, check for later last_modified for tile already loaded.
        - If so, use that tile, even if hash does not match. 
        - If new tile is later, tile has changed, generate the image file for it.
      - If match, and no UUID, generate. There's no existing asset.
    - Generation includes making a tile_asset row with no UUID but all other fields.
      
      At upload time, update items with no UUID, overwriting when we have an upload with a UUID.
      - What if there's a UUID already?
      
      New step: garbage collection. After successful upload and a new region_impostors file,
      remove all the unused tile entries.
      
      Progress:
      - Moved common code to tileassets.
      TODO:
      - generate new format impostor names - only need ident info, not water level, etc. 
      
2026-02-23
    Partway through new plan.
    - Finish generateterrain fixes. Not clear what happens on insert of duplicate entry for tile.
      - Generate texture object if we don't have a UUID yet?
        - Yes, but only if we don't have a UUID. Move insert tile check to after check for already having a UUID.
      
    - Need to fix uploadimpostors to set UUIDs in tiles.
    - All UUIDs are set in the upload impostors missing UUID fixup? Slow.
    - Do we need update_tile any more?
    
2026-02-24
    Generate side works, now need to do uploadimpostor side.
    - Don't really need all that info encoded in name. Just x, y, sx, sy, lod, hash.
      - But keep the name format for now. Change later, maybe.
      
2026-02-25
    Updating UUIDs in tiles.
    - tile_asssets get_faces_json will generate the JSON with UUIDs. Use that in updateimpostors.
     - Apply that to initial_impostor which matches loc data for incoming impostor.
    - sculpt UUID in initial_impostor needs to be updated.
     - use add_impostor?
       - No, don't have height field any more.
       - Need new fn that just updates UUID - insert_uuid
         - Just needs key info and UUID.
         
2026-02-28
     Working, but sometimes sculpt_hash is null in impostors. Should never happen.
     - Trouble when two tiles at same loc but different viz group? Check unique key in SQL def. 
       - Get rid of UNIQUE INDEX (grid, asset_name).
     - This one shows up in impostors with a null sculpt hash. Why?
     05:25:13 [DEBUG] (1) generateterrain: Region impostor data: RegionImpostorData { region_loc: [460800, 305152], region_size: [2048, 2048], scale: [2048.0, 2048.0, 34.994167], impostor_lod: 3, viz_group: 1, sculpt_uuid: Some(0453c546-ed8d-2f7f-94e4-5a7eab363271), sculpt_hash: Some("e5ce320f"), mesh_uuid: None, mesh_hash: None, elevation_offset: 0.0, water_height: Some(34.5), name: Some("LOD3 (460800, 305152)"), grid: "agni", faces: [RegionImpostorFaceData { base_texture_uuid: None, emissive_texture_uuid: None, base_texture_hash: "40a7ee78", emissive_texture_hash: None }] }
     05:25:13 [DEBUG] (1) common::initialimpostors: Inserting impostor into initial_impostors, params: Named({"mesh_uuid": Null, "faces_json": Bytes("[{\"base_.."), "name": Bytes("LOD3 (46.."), "scale_x": Float(2048.0), "region_size_y": UInt(2048), "region_loc_y": UInt(305152), "scale_y": Float(2048.0), "elevation_offset": Float(0.0), "sculpt_uuid": Bytes("0453c546.."), "region_size_x": UInt(2048), "viz_group": UInt(1), "water_height": Float(34.5), "region_loc_x": UInt(460800), "mesh_hash": Null, "sculpt_hash": Bytes("e5ce320f"), "scale_z": Float(34.994167), "grid": Bytes("agni"), "impostor_lod": UInt(3)})
     - That's valid, and an insert was done with that data. But in region_impostors, sculpt_hash is Null, while all other data is correct.
     - SQL in initialimpostors looks OK for sculpt_hash field.
     - What else could overwrite this?
     - It's downloadimpostor - the hash fields are not read from the database and placed into the JSON. 
       - Fields are OK in the database, but not copied to the output JSON.
       - The viewer doesn't use them. So, harmless but probably should be fixed.
       
    Loaded up about 120 regions from Corsica.
    - Crash in runaway EOF detection. Increased counter from 100 to 1000. Recheck; that's not a permanent fix.
    - Crash in updateimpostors at let mut tiles_missing_uuids = conn.exec_map(
            SQL_SELECT_MISSING_TILE,
      Not just an error return, a hard crash.
    - There are many initial impostors with no UUID. Why?
      - There are tile_assets with no sculpt UUID. Why?
        - The whole AssetUploadArrayShort thing is wrong. Should be x, y, sx, sy, lod, hash, uuid.
        
2026-03-01
   
    Working for Celchu and vicinity.
    - Sculpt sizes are off. Gaps at edges. [FIXED?]
    
    UUID update not working:
    04:06:59 [WARN] insert_texture_uuid_for_tile: update did not change JSON: params: Named({"viz_group": UInt(2), "region_size_x": UInt(256), "grid": Bytes("agni"), "region_size_y": UInt(256), "region_loc_y": UInt(306944), "impostor_lod": UInt(0), "region_loc_x": UInt(462592), "faces_json": Bytes("[{\"base_..")}), 
        before: [{"base_texture_hash": "26511065", "base_texture_uuid": null, "emissive_texture_hash": null, "emissive_texture_uuid": null}], 
        after: [{"base_texture_uuid":"cafe48f1-40a9-f8e3-5f2b-b61415a74014","emissive_texture_uuid":null,"base_texture_hash":"26511065","emissive_texture_hash":null}]
        
Should have updated UUID but did not:
        
        05:30:09 [DEBUG] (1) uploadimpostor: Updating tile: AssetUpload { asset_name: "RT0_462592_306688_256_256_3.55_31.44_0_0_34.50_75f86e9b", asset_hash: "75f86e9b", region_loc: [462592, 306688], region_size: [256, 256], grid: "agni", asset_uuid: Some("e7b8c9a9-4f38-d38a-2680-19b91711989f"), elevation_offset: 31.44, scale: [256.0, 256.0, 3.55], water_height: 34.5, impostor_lod: 0, tile_asset_type: BaseTexture(0) }
05:30:09 [DEBUG] (1) common::tileassets: Insert UUID params: Named({"region_loc_y": UInt(306688), "impostor_lod": UInt(0), "asset_uuid": Bytes("e7b8c9a9.."), "region_loc_x": UInt(462592), "texture_index": Null, "asset_hash": Bytes("75f86e9b"), "grid": Bytes("agni"), "asset_type": Bytes("BaseText..")})
05:30:09 [DEBUG] (1) common::tileassets: Tile asset UUID update succeeded. Rows: None, params Named({"asset_hash": Bytes("75f86e9b"), "asset_uuid": Bytes("e7b8c9a9.."), "region_loc_y": UInt(306688), "region_loc_x": UInt(462592), "impostor_lod": UInt(0), "asset_type": Bytes("BaseText.."), "grid": Bytes("agni"), "texture_index": Null})
05:30:09 [INFO] Tile UUID unchanged for AssetUpload { asset_name: "RT0_462592_306688_256_256_3.55_31.44_0_0_34.50_75f86e9b", asset_hash: "75f86e9b", region_loc: [462592, 306688], region_size: [256, 256], grid: "agni", asset_uuid: Some("e7b8c9a9-4f38-d38a-2680-19b91711989f"), elevation_offset: 31.44, scale: [256.0, 256.0, 3.55], water_height: 34.5, impostor_lod: 0, tile_asset_type: BaseTexture(0) }
05:30:09 [DEBUG] (1) uploadimpostor: Inserting UUID in initial_impostors: AssetUpload { asset_name: "RT0_462592_306688_256_256_3.55_31.44_0_0_34.50_75f86e9b", asset_hash: "75f86e9b", region_loc: [462592, 306688], region_size: [256, 256], grid: "agni", asset_uuid: Some("e7b8c9a9-4f38-d38a-2680-19b91711989f"), elevation_offset: 31.44, scale: [256.0, 256.0, 3.55], water_height: 34.5, impostor_lod: 0, tile_asset_type: BaseTexture(0) }

No rows changed on UUID insert. Why?
- Texture index NULL. Fixed that.

Failed UUID lookup:

7:01:02 [DEBUG] (1) common::initialimpostors: Looking up tile UUID: Named({"region_loc_y": UInt(279808), "grid": Bytes("agni"), "asset_hash": Bytes("e3b0651d"), "region_loc_x": UInt(290816), "asset_type": Bytes("BaseText.."), "impostor_lod": UInt(0)})
07:01:02 [DEBUG] (1) common::initialimpostors: Looked up tile UUID: Some(None)
- New File is in R00. But the database entry is older than the current entry, and the old entry has no UUID.
  Hash of new file is 3feb4f53 No match.
  
  Insert of UUID did nothing:
  
  05:29:39 [DEBUG] (1) uploadimpostor: Updating tile: AssetUpload { asset_name: "RT0_290816_279808_256_256_54.47_36.46_0_0_20.00_3feb4f53", asset_hash: "3feb4f53", region_loc: [290816, 279808], region_size: [256, 256], grid: "agni", asset_uuid: Some("b0f188c1-ff53-8269-0050-b089f0c7323b"), elevation_offset: 36.46, scale: [256.0, 256.0, 54.47], water_height: 20.0, impostor_lod: 0, tile_asset_type: BaseTexture(0) }
05:29:39 [DEBUG] (1) common::tileassets: Insert UUID params: Named({"asset_uuid": Bytes("b0f188c1.."), "asset_hash": Bytes("3feb4f53"), "region_loc_x": UInt(290816), "impostor_lod": UInt(0), "region_loc_y": UInt(279808), "grid": Bytes("agni"), "texture_index": Null, "asset_type": Bytes("BaseText..")})
05:29:39 [DEBUG] (1) common::tileassets: Tile asset UUID update succeeded. Rows: None, params Named({"asset_hash": Bytes("3feb4f53"), "asset_type": Bytes("BaseText.."), "region_loc_y": UInt(279808), "grid": Bytes("agni"), "region_loc_x": UInt(290816), "texture_index": Null, "asset_uuid": Bytes("b0f188c1.."), "impostor_lod": UInt(0)})

Oh, right, we fixed texture index null insertion, but did not rerun generate, where that fix did something.
Fix tomorrow.

2026-03-04

   No, it's not fixed.
   - in SQL, NULL = NULL evaluates to UNKNOWN. [FIXED]
   - Row count is obtained with another call on UPDATE. [FIXED]
   
2026-03-08

   More problems.
   - Sculpts need longer skirts.
   - Changed sculpt generation but no new sculpts generated. Why?
     - Because they were known asset from a previous run? Probably
       - Need garbage collector.
   - Texture is misaligned vs. sculpt UVs.
   
2026-03-10

   What's wrong here?
   Image is 256x256.
   Sculpt is 64x64, increased from 32x32.
   At 32x32 scale, one extra pixel on each edge is used for the vertical side.
   At 64x64 scale, two extra pixels on each edge.
   So image needs to be reduced by a factor of 2/32, or 1/16, to fit the UVs of the sculpt.
   For an image 256x256, 1/16th of that is 16 pixels total, or 8 pixels on each side.
   
   This is a failure in image hashing/dup check. The image being used for this sculpt isn't even the right size. 
   But the hash matches after regenerating it. Huh?
   04:58:40 [INFO] Sculpt image asset already exists: RS_290560_279552_256_256_34.32_45.06_0_0_20.00_1cccb6a3 UUID: 6bdc6f3f-930d-43f4-5acb-b6d7461e847b
   This asset is bad. But re-generating it seems to produce the same hash. Asserts are checking the size of the new version. How?
   This seems to be an old asset version which should have been replaced after the code that generates the sculpt was fixed. 
   
2026-03-12

   There's some kind of off by one error with sculpt mapping for impostors.
   - It's not in Sharpview. It can be reproduced by making sculpts in Firestorm.
   - The scaled images with the outer 8 pixels (of 256) as the sides do not map
     correctly to the sculpt texture UVs. Looks like an off by 1 error.
     
2026-03-13
   Better alignment but still not perfect. 
   - Offset in add_perimeter_to_image is wrong - not centered.
   
2026-03-14
   Now off by about half a road width.
   Also, theres a new vertical problem. Sculpts are too high by a few meters.
   - Scheme for adding longer skirts to sculpts produced misadjusted offset. Needs fix.
   - Tried indenting the images by 8, 10, and 16 PERIMETER_PIXELS. 8 and 10 look about the same. 16 is bad on some edges. Back to 8.
   - Looks good at 8. Some of the artifacts are in the original map tiles.
   - Now just need to fix the Z axis problem. 
     - Elevation offset in height field is from the actual input measurements.
     - tile_assets only has elevation offset encoded in the name.
       - Is that used?
         - No. Get elevation_offset out of AssetUpload. Maybe out of name?
     - elevation_offset in impostors needs to be adjusted for the skirts of the sculpt.
       - Won't be the same as the one from the height field any more.
     - When the sculpt is built, zmin is the bottom of the skirt.
     - assemble_region_impostor_data needs the adjusted elevation_offset, not the one it gets from the height field.
       - Need scale and offset from TerrainSculpt, not height field.
         - TerrainSculpt ought to be a trait, so getting scale and offset is generic.
       - Only thing needed from height field is water level.
         - That needs to be somewhere else. Where?
         - It's a legit part of height_field, because, for multi-region height fields, it needs to be the minimum.
           - And terrain height needs to be raised to water level for multi-region height fields?
         - AssetUpload::new needs modified scale and offset, which height field does not have.
           - It can't use TerrainGeometry because that's also used for non-terrain textures.
           - Bleah. More plumbing trouble.
           
2026-03-16
     Fixed plumbing.
     TerrainSculpt::makeimage is wrong.
     Want to add skirt to each image.
     Increase all Z values by SKIRT_HEIGHT
     Increase zmax by SKIRT_HEIGHT
     Keep same zmin?     
     Decrease offset by SKIRT_HEIGHT
     
2026-03-17
     
     So how did we get these numbers:
     05:46:12 [DEBUG] (1) common::uploadedregioninfo: New height field, scale 70.19521, offset 11.9384
     - Those values came in at upload. 
     05:46:12 [INFO] Generating sculpt for "Chalmun": HeightField samples (65, 65)  region (256, 256)
     05:46:12 [DEBUG] (1) common::uploadedregioninfo: Height range:  11.9384 .. 81.85941
     - This below comes from processing the 0..255 encoded height data. Low is legit, high is smaller than it should be.
     - Off by 2.81 out of 70. That''s huge, even after going through the 256-value coding.
     05:46:12 [DEBUG] (1) generateterrain::sculptmaker: Z bounds: 7.94 to 79.67
     
2026-03-18
     New sculpts not being used because offset and scale are not part of hash. Fix. [DONE]
     Vertical position is still wrong? What did I do wrong?
     - pub fn assemble_region_impostor_data(terrain_geometry: &dyn TerrainGeometry, region: &RegionData, height_field: &HeightField, viz_group: u32, 
        asset_hash: &str, asset_uuid_opt: Option<Uuid>, face_data: &[RegionImpostorFaceData]) -> RegionImpostorData {
     is getting the range and offset from the height field, not the terrain geometry.
     
2026-03-20

     Cleanup. Next big thing to write is a garbage collector for UUIDs.
     
2026-03-31

     Garbage collector finished, more cleanup. Need a full test run.
     
2026-04-06

     Integrated BonnieBots data and am testing vizgroups. Mostly
     correct, except for "Soryn" region.
     
     04:24:23 ^[[0m^[[36m[DEBUG] ^[[0m(2) generateterrain::vizgroup: Blocks with different viz groups touch: "Soryn" (177152, 315648) and "Noble Dreams" (177408, 315904)
     04:24:23 ^[[0m^[[36m[DEBUG] ^[[0m(2) generateterrain::vizgroup: Merged: 2 live blocks weak, 2 regions

2026-04-12
    Running, but clip regions seem slightly off.
    Sculpt Scale-down is broken.
    
2026-04-13
    Working for Blake Sea and Corsica using BonnieBots data.
    Aborts on Heterocera due to a 403 error trying to read a
    LOD 9 texture, which is not supported as too large. Need
    to work around that.
    Three regions in Heterocera BonnieBots can't scan at (128,128):
    "Dierli", "Zeuzera",  and "Ambulyx" 
    
2026-04-14
    get_enclosing_square result is bogus.
    - Does not consider the size of the rectangle for the viz group at all.
    
2026-04-18
    Now combining large map tiles when necessary, to get past SL limit.
    But this is not enough. It can result in requests for unused tiles.
    If we get a 403 or 404 error for a tile for an LOD > 0, we must
    make a water tile.
    
2026-04-20

    Insanely slow generating large water areas.
    - The recursive descent process for big areas of water is way too slow.
    - First fix, RC everything and use a cheaper resize algorithm.

    thread 'main' (390796) panicked at src/generator/generateterrain.rs:868:17:
    Failed: CodecError { Packets out of sync }
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    
    It's a MySQL error. Kept the connection open and idle for too long?
    Bug report sent.
    
2026-04-21

    Fixed performance problem. Ran to completion in about 10 mins.
    Edges of sculpts are not behing handled right for large tiles. Check.
    Probably because they are 1024x1024.
    
2026-04-22

    Need new matching algorithm in Sharpview.
    - Get appropriate viz group (called estate ID in Sharpview; fix.)
    - Find largest tile. 
      - There should only be one, if generateterrain generated this info. But allow for multiple.
      - If current pos is in largest tile, 
        - Generate tiles below that tile.
        - Otherwise, log as no impostors available.
    - This is equivalent to "generate entire viz group". So why isn't the existing code working?
    
2026-04-23

    Problem is in generator.
    - 8K and above tiles are never generated.
    - Handled incorrectly as "all water".
    - build_tiles_with_land does not seem to be marking some tiles.
    
2026-04-24.

    Success! Can show Heterocera.
    
    Problems:
    - Generateterrain generates LOD 11, but viewer rejects the file as having an excessive LOD. Fix in viewer or generator? [FIXED IN VIEWER]
    - Water texture scaling is wrong. [FIXED IN VIEWER]
    
2026-05-05

    Viewer is now ready for entries with water only, no terrain. Need to generate them.
    Crashes when BonnieBots data lacks a map tile. [FIXED]
    
2026-05-06

    What's the water height for a water-only tile?
    - No real data source for this.
    - Want adjacent land tile water level, probably.
      - How to get that efficiently?
      - Adjacent tiles might be all water, too.
        - How to find relevant land?
        - Construct index of all LOD0 tiles. Find nearest.
          - https://crates.io/crates/kd-tree
        - Height info is expensive to get. HTTP request for each one, if done separately.
          - Collect height while generating tiles, queue up water-only tile items to do at end.
          
2026-05-08
   Water only tile code is in but no water only tiles are being found for Heterocera.
   - Too many new texture tiles are generated
   - 00:54:07 [WARN] Duplicate hashes for grid agni looking up SculptTexture asset at [290304, 268544] size [256, 256] 
     - There are duplicate tile assets because NULL != NULL, so this doesn't work:
       UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, asset_hash, texture_index, asset_type)
       
2026-05-14
    Fixed duplicate hashes.
    Still not finding water-only tiles to be generated as water-only, no texture file.
    - Does regionorder.rs actually generate them?
      - No. They are filtered out at line 478, and not pushed to output.
        - Should they be?
        - This thing is way too complicated.


2026-05-16
   Moved water only tile generation to viewer. Simpler. Less data in database.
