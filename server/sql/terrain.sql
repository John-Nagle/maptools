-- In database "terrain"

CREATE TABLE IF NOT EXISTS raw_terrain_heights (
    -- x and y are the coordinates of the southwest corner of the region, its zero pont.
    x INT NOT NULL,
    y INT NOT NULL,
    -- region name, for search,
    name VARCHAR(100) NOT NULL,    
    -- the raw JSON from a SL script.
    height_data: JSON NOT NULL,
    -- UUID of creator
    creator: CHAR(36) NOT NULL,
    creation_time: TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- UUID of confirmer - attempted upload with same JSON.
    confirmer: CHAR(36),
    confirmation_time: TIMESTAMP,
    UNIQUE INDEX (x,y),
    INDEX(name)
    )
