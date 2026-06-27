-- Backtick-quoted identifiers: spaces, doubled-backtick escape, in select/alias/table positions.
SELECT `user id`, `it``s ok` AS `weird``name`, c.amount
FROM `main`.`default`.`order events` AS c
WHERE `user id` > 0;
