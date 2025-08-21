-- In database "terrain"

CREATE TABLE IF NOT EXISTS raw_terrain_heights (
    x INT NOT NULL,
    y INT NOT NULL,
    name VARCHAR(100) NOT NULL,    
    height_data JSON NOT NULL,
    creator CHAR(36) NOT NULL,
    creation_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    confirmer CHAR(36),
    confirmation_time TIMESTAMP,
    UNIQUE INDEX (x,y),
    INDEX(name)
    )
