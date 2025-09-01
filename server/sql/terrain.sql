-- In database "terrain"

CREATE TABLE IF NOT EXISTS raw_terrain_heights (
    grid VARCHAR(40) NOT NULL,
    region_coords_x INT NOT NULL,
    region_coords_y INT NOT NULL,
    size_x INT NOT NULL,
    size_y INT NOT NULL,
    name VARCHAR(100) NOT NULL,
    scale FLOAT NOT NULL,
    offset FLOAT NOT NULL,
    elevs JSON NOT NULL,   
    water_level FLOAT NOT NULL,
    creator VARCHAR(63) NOT NULL,
    creation_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    confirmer VARCHAR(63) DEFAULT NULL,
    confirmation_time TIMESTAMP DEFAULT NULL,
    UNIQUE INDEX (grid, region_coords_x, region_coords_y),
    INDEX(name)
    )
