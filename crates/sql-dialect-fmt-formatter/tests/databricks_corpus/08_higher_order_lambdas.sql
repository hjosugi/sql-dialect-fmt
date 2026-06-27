-- Higher-order functions with single- and multi-param lambdas and nesting.
SELECT
    transform(xs, x -> x + 1) AS inc,
    filter(xs, x -> x > 0) AS positives,
    aggregate(xs, 0, (acc, x) -> acc + x) AS total,
    zip_with(a, b, (x, y) -> x + y) AS zipped,
    transform(xss, xs -> transform(xs, x -> x * 2)) AS nested
FROM main.default.arrays;
