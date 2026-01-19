-- In database "terrain"

-- Raw terrain heights. Updated by an LSL script that
-- visits regions.

CREATE TABLE IF NOT EXISTS raw_terrain_heights (
    grid VARCHAR(40) NOT NULL,
    region_coords_x INT NOT NULL,
    region_coords_y INT NOT NULL,
    size_x INT NOT NULL,
    size_y INT NOT NULL,
    name VARCHAR(100) NOT NULL,
    scale FLOAT NOT NULL,
    offset FLOAT NOT NULL,
    samples_x INT NOT NULL,
    samples_y INT NOT NULL,
    elevs MEDIUMBLOB NOT NULL,   
    water_level FLOAT NOT NULL,
    creator VARCHAR(63) NOT NULL,
    creation_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    confirmer VARCHAR(63) DEFAULT NULL,
    confirmation_time TIMESTAMP DEFAULT NULL,
    UNIQUE INDEX (grid, region_coords_x, region_coords_y),
    INDEX(name)
    )
    
   
-- Impostor information. What the viewer needs to draw an impostor.
 
CREATE TABLE IF NOT EXISTS region_impostors (
    grid VARCHAR(40) NOT NULL,
    name VARCHAR(100) NOT NULL,
    region_loc_x INT NOT NULL,
    region_loc_y INT NOT NULL,
    region_size_x INT NOT NULL,
    region_size_y INT NOT NULL,
    scale_x INT NOT NULL,
    scale_y INT NOT NULL,
    scale_z FLOAT NOT NULL,
    elevation_offset FLOAT NOT NULL,
    impostor_lod TINYINT NOT NULL,
    viz_group INT NOT NULL,
    mesh_uuid CHAR(36) DEFAULT NULL,
    mesh_hash CHAR(16) DEFAULT NULL,
    sculpt_uuid CHAR(36) DEFAULT NULL,
    sculpt_hash CHAR(16) DEFAULT NULL,
    water_height FLOAT NOT NULL,
    creator VARCHAR(63) NOT NULL,
    creation_time TIMESTAMP NOT NULL,
    faces_json JSON NOT NULL,
    UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod),
    INDEX(grid, viz_group),
    INDEX(name)
)
