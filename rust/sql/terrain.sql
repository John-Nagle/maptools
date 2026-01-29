-- In database "terrain"

-- Raw terrain heights. Updated by an LSL script that
-- visits regions.

CREATE TABLE IF NOT EXISTS raw_terrain_heights (
    grid VARCHAR(40) NOT NULL,
    region_loc_x INT NOT NULL,
    region_loc_y INT NOT NULL,
    region_size_x INT NOT NULL,
    region_size_y INT NOT NULL,
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
    UNIQUE INDEX (grid, region_loc_x, region_loc_y),
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
    uniqueness_viz_group INT DEFAULT NULL,
    mesh_uuid CHAR(36) DEFAULT NULL,
    mesh_hash CHAR(8) DEFAULT NULL,
    sculpt_uuid CHAR(36) DEFAULT NULL,
    sculpt_hash CHAR(8) DEFAULT NULL,
    water_height FLOAT NOT NULL,
    creator VARCHAR(63) NOT NULL,
    creation_time TIMESTAMP NOT NULL,
    faces_json JSON NOT NULL,
    UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, uniqueness_vizgroup),
    INDEX(grid, viz_group),
    INDEX(name)
)

--- Region textures. Used to hold texture information which needs to be matched to geometry.

CREATE TABLE IF NOT EXISTS tile_asset (
    grid VARCHAR(40) NOT NULL,
    region_loc_x INT NOT NULL,
    region_loc_y INT NOT NULL,
    region_size_x INT NOT NULL,
    region_size_y INT NOT NULL,
    impostor_lod TINYINT NOT NULL,
    viz_group INT NOT NULL,
    file_name VARCHAR(63) NOT NULL,
    asset_type ENUM('BaseTexture', 'EmissiveTexture', 'SculptTexture', 'Mesh') NOT NULL DEFAULT 'BaseTexture',
    texture_index SMALLINT DEFAULT NULL,
    asset_uuid CHAR(36) NOT NULL,  
    asset_hash CHAR(8) NOT NULL,
    creation_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, viz_group, texture_index),
    UNIQUE INDEX (file_name)
)
