-- GROUP BY CUBE / ROLLUP / GROUPING SETS.
SELECT region, product, sum(amount)
FROM main.default.sales
GROUP BY cube(region, product);
