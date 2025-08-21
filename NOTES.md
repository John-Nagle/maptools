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
      
      
