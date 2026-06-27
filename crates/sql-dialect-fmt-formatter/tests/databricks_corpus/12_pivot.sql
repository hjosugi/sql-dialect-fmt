-- PIVOT.
SELECT *
FROM main.default.sales PIVOT (sum(amount) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4'));
