CREATE TABLE sbtest1 (
    id  INTEGER NOT NULL,
    k   INTEGER DEFAULT '0' NOT NULL,
    PRIMARY KEY (id)
)
AS SELECT
    rownum,
    rownum
FROM rand;
