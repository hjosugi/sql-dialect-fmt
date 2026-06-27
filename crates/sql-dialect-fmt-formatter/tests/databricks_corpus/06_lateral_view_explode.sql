-- LATERAL VIEW explode / posexplode, including OUTER.
SELECT e.id, item, pos
FROM main.default.events AS e
LATERAL VIEW explode(e.items) t AS item
LATERAL VIEW OUTER posexplode(e.tags) p AS pos, tag;
