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
  
2023-09-26

   Data collection looks good. Manually generated sculpts look good.
   Need some kind of authentication for the capture script.
   
   Next steps: generate a JSON file for the viewer automatically.
   - Need a database first.
   - Where do we get UUIDs?
     - Upload script - finds all sculpt images and adds their UUIDs.
   - Generate large-area merged sculpts.
   
   - Got sculpt display into Sharpview.
   - Sculpts are being generated with X and Y exchanged.
   
2023-09-29

   Sculpt impostors working.
   Need to generate sculpts with max, instead of min, so rocks
   in water are above the water line. [DONE]
   
2023-10-02

   Wrote LSL script to look at uploaded sculpt textures and collect
   info to send to server. Wrote SQL for region_impostor table.
   Need to write Rust server to handle uploads and queries.
   - Queries needed "what viz_group is this region in", 
     "give me all JSON for this viz_group", and "give me all JSON".
   - Upload needs some thought.
     - Need authentication for upload.
     
2023-10-04

   Beginning of impostor upload code.
   Find out why "Siren's Isle" came through as "Sirens".
   Parse enums like this: https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=3c43bd9041eb567dbf974c8519ef10d7
   
   
      
      
