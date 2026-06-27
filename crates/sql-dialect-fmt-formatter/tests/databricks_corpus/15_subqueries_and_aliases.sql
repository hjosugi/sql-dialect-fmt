-- Subqueries, derived tables, IN/EXISTS predicates with backtick aliases.
SELECT t.`user id`, t.total
FROM (
    SELECT user_id AS `user id`, sum(amount) AS total
    FROM main.default.events
    GROUP BY user_id
) AS t
WHERE t.`user id` IN (SELECT id FROM main.default.active_users);
