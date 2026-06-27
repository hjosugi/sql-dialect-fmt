-- Unity Catalog three-level names in select list, FROM, and joins.
SELECT prod.sales.orders.order_id, u.name
FROM prod.sales.orders
JOIN prod.identity.users AS u ON prod.sales.orders.user_id = u.id;
